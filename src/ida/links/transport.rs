//! KE HTTP connections: validated addresses, ordered failover and redirect checks.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};

mod addresses;
use crate::util::cookies;
mod proxy;
mod tls;

const IO_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REDIRECTS: usize = 5;

#[derive(Clone)]
pub(super) struct Transport {
    origin: url::Origin,
    attempts: Vec<Attempt>,
    pinned: bool,
    session: Session,
}

#[derive(Clone)]
struct Attempt {
    client: reqwest::Client,
    cookie_ip: Option<IpAddr>,
}

#[derive(Clone)]
struct Session {
    cookies: Arc<Mutex<cookies::Jar>>,
    allow_private: bool,
    timeout: Duration,
    proxies: proxy::Routing,
    tls: tls::Contexts,
}

impl Session {
    fn new(allow_private: bool, timeout: Duration) -> Self {
        Self {
            cookies: Arc::new(Mutex::new(cookies::Jar::default())),
            allow_private,
            timeout,
            proxies: proxy::Routing::default(),
            tls: tls::Contexts::default(),
        }
    }
}

impl Transport {
    /// Resolve and validate before the caller performs any cache mutations.
    pub(super) async fn for_url(url: &url::Url) -> Result<Self> {
        let mut session =
            Session::new(crate::config::Env::global().ke.allow_private_hosts, IO_TIMEOUT);
        session.proxies = proxy::Routing::from_environment()?;
        session.tls = tls::Contexts::from_environment()?;
        Self::prepare(url, session).await
    }

    async fn prepare(url: &url::Url, session: Session) -> Result<Self> {
        let host = hostname(url)?;
        let addresses = if session.allow_private {
            None
        } else {
            let port = url.port_or_known_default().expect("HTTP(S) has a default port");
            let resolved = tokio::net::lookup_host((host.as_str(), port)).await?.collect();
            Some(addresses::validate(resolved)?)
        };
        Self::from_addresses(url, addresses, session)
    }

    #[cfg(test)]
    fn with_addresses(
        url: &url::Url,
        addresses: Option<Vec<SocketAddr>>,
        allow_private: bool,
        timeout: Duration,
    ) -> Result<Self> {
        Self::from_addresses(url, addresses, Session::new(allow_private, timeout))
    }

    fn from_addresses(
        url: &url::Url,
        addresses: Option<Vec<SocketAddr>>,
        session: Session,
    ) -> Result<Self> {
        let host = hostname(url)?;
        let pinned = addresses.is_some();
        let candidates = match addresses {
            Some(addresses) => addresses.into_iter().map(Some).collect(),
            None => vec![None],
        };
        let mut attempts = Vec::new();
        for address in candidates {
            let mut builder = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .no_proxy()
                .no_gzip()
                .no_deflate()
                .no_brotli()
                .no_zstd()
                .use_preconfigured_tls(session.tls.origin.as_ref().clone())
                .connect_timeout(session.timeout)
                .read_timeout(session.timeout);
            if let Some(address) = address {
                builder = builder.resolve_to_addrs(&host, &[address]);
            }
            attempts.push(Attempt {
                client: builder.build()?,
                cookie_ip: address.map(|address| address.ip()),
            });
        }
        Ok(Self {
            origin: url.origin(),
            attempts,
            pinned,
            session,
        })
    }

    async fn get(&self, url: &url::Url) -> Result<reqwest::Response> {
        if url.origin() != self.origin {
            return Err(Error::Other(
                "KE transport origin does not match its validated host".into(),
            ));
        }
        let mut request_url = url.clone();
        if self.pinned {
            // Upstream's IP-authority rewrite drops userinfo. The original host
            // remains in our URL for Host, TLS SNI and certificate verification.
            request_url.set_username("").expect("validated HTTP(S) URL");
            request_url.set_password(None).expect("validated HTTP(S) URL");
        }
        let mut last_error = None;
        for attempt in &self.attempts {
            let mut cookie_url = request_url.clone();
            if let Some(ip) = attempt.cookie_ip {
                cookie_url.set_ip_host(ip).expect("validated HTTP(S) URL");
            }
            let cookie = self.session.cookies.lock().unwrap().header(&cookie_url)?;
            let result = if self.session.proxies.selected(&cookie_url) {
                self.session
                    .proxies
                    .get(
                        &request_url,
                        &cookie_url,
                        self.pinned,
                        cookie,
                        &self.session.tls,
                        self.session.timeout,
                    )
                    .await
            } else {
                let mut request = attempt
                    .client
                    .get(request_url.clone())
                    .header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate");
                if let Some(cookie) = cookie {
                    request = request.header(reqwest::header::COOKIE, cookie);
                }
                request.send().await.map_err(|error| proxy::Failure {
                    connect: error.is_connect(),
                    error: error.into(),
                })
            };
            match result {
                Ok(response) => {
                    self.session.cookies.lock().unwrap().store(response.headers(), &cookie_url);
                    return Ok(response);
                }
                Err(failure) if failure.connect => last_error = Some(failure.error),
                Err(failure) => return Err(failure.error),
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Err(Error::Other("KE host has no validated connection addresses".into())),
        }
    }

    pub(super) async fn response(&self, url: &url::Url) -> Result<reqwest::Response> {
        let mut current = url.clone();
        let mut transport = self.clone();
        for _ in 0..=MAX_REDIRECTS {
            let response = transport.get(&current).await?;
            if let Some(target) = redirect(&current, &response)? {
                drop(response);
                transport = Self::prepare(&target, self.session.clone()).await?;
                current = target;
                continue;
            }
            if response.status() != reqwest::StatusCode::OK {
                return Err(Error::Other(format!(
                    "KE download failed: HTTP {}",
                    response.status().as_u16()
                )));
            }
            return Ok(response);
        }
        Err(Error::Other("too many KE redirects".into()))
    }
}

fn hostname(url: &url::Url) -> Result<String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::Other("invalid KE content URL: expected HTTP(S)".into()));
    }
    // The resolver needs an IPv6 address without URL authority brackets.
    match url.host() {
        Some(url::Host::Domain(host)) => Ok(host.to_owned()),
        Some(url::Host::Ipv4(ip)) => Ok(ip.to_string()),
        Some(url::Host::Ipv6(ip)) => Ok(ip.to_string()),
        None => Err(Error::Other("missing KE content host".into())),
    }
}

fn redirect(url: &url::Url, response: &reqwest::Response) -> Result<Option<url::Url>> {
    if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
        return Ok(None);
    };
    let location = location.to_str().map_err(|error| Error::Other(error.to_string()))?;
    Ok(Some(url.join(location).map_err(|error| Error::Other(error.to_string()))?))
}

#[cfg(test)]
mod tests;
