//! Independent legacy/Netscape batches are constructed before any insertion.

use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive};
use reqwest::header::{HeaderMap, SET_COOKIE};

use crate::util::http_headers::TextDecoder;

use super::parse::{self, Format};
use super::{Cookie, Jar, scope};

impl Jar {
    pub(super) fn extract(&mut self, headers: &HeaderMap, url: &url::Url, now: i64) {
        // DefaultCookiePolicy disables RFC 2965. CookieJar nevertheless parses
        // Set-Cookie2 when any Set-Cookie header accompanies it, even an empty one.
        if !headers.contains_key(SET_COOKIE) {
            return;
        }
        let context = scope::Context::new(url);
        let decoder = TextDecoder::new(headers);
        let legacy = headers
            .get_all("set-cookie2")
            .iter()
            .flat_map(|header| parse::split(&decoder.decode(header)))
            .collect();
        let mut pending =
            self.construct(legacy, &context, now, Format::Rfc2965).unwrap_or_default();

        let year = chrono::DateTime::from_timestamp(now, 0)
            .map(|time| chrono::Datelike::year(&time.with_timezone(&chrono::Local)))
            .unwrap_or(1970);
        let netscape: Result<Vec<_>, ()> = headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|header| parse::attributes(&decoder.decode(header), year))
            .collect();
        if let Ok(attributes) = netscape {
            pending.extend(
                self.construct(attributes, &context, now, Format::Netscape).unwrap_or_default(),
            );
        }
        // Both formats can delete stored values while being constructed. Their
        // pending values are inserted afterward, with Netscape values last.
        for cookie in pending {
            if !scope::accepts(&cookie, &context) {
                continue;
            }
            self.domains
                .entry(cookie.domain.clone())
                .or_default()
                .entry(cookie.path.clone())
                .or_default()
                .insert(cookie.name.clone(), cookie);
        }
    }

    fn construct(
        &mut self,
        attributes: Vec<Vec<parse::Attribute>>,
        context: &scope::Context,
        now: i64,
        format: Format,
    ) -> Result<Vec<Cookie>, ()> {
        let normalized: Result<Vec<_>, ()> = attributes
            .into_iter()
            .map(|attributes| parse::Normalized::parse(attributes, now, format))
            .collect();
        let now = BigInt::from(now);
        let mut pending = Vec::new();
        for normalized in normalized?.into_iter().flatten() {
            let Some(mut cookie) = normalized.into_cookie(context, format)? else {
                continue;
            };
            if cookie.expires.as_ref().is_some_and(|expires| expires <= &now) {
                if let Some(names) = self
                    .domains
                    .get_mut(&cookie.domain)
                    .and_then(|paths| paths.get_mut(&cookie.path))
                {
                    names.shift_remove(&cookie.name);
                }
                continue;
            }
            if let Some(expires) = cookie.expires {
                // Cookie.__init__ uses int(float(expires)). Overflow cancels
                // this format's pending cookies, preserving earlier deletions.
                cookie.expires = Some(expires.to_f64().and_then(BigInt::from_f64).ok_or(())?);
            }
            pending.push(cookie);
        }
        Ok(pending)
    }
}
