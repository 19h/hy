//! Stream signed uploads and consume responses before confirming publication.

use std::path::Path;

use reqwest::header;

use super::ApiClient;
use crate::error::Result;

impl ApiClient {
    pub async fn put_file(&self, url: &str, file_path: &Path) -> Result<()> {
        use futures_util::TryStreamExt;

        let meta = std::fs::metadata(file_path)?;
        let file_size = meta.len();

        let content_type = match file_path.extension().and_then(|e| e.to_str()) {
            Some("zip") => "application/zip",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };

        let pb = crate::util::tui::byte_progress(
            file_size,
            format!("Uploading {}", file_path.file_name().unwrap_or_default().to_string_lossy()),
        );

        // Stream the file so progress is real and memory stays flat.
        let file = tokio::fs::File::open(file_path).await?;
        let pb_stream = pb.clone();
        let stream = tokio_util::io::ReaderStream::new(file).inspect_ok(move |chunk| {
            pb_stream.inc(chunk.len() as u64);
        });

        let request = self
            .inner
            .put(url)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, file_size)
            .body(reqwest::Body::wrap_stream(stream))
            .build()?;
        let resp = self.send_following(request).await?;
        Self::handle(resp).await?.bytes().await?;

        pb.finish_and_clear();
        Ok(())
    }
}
