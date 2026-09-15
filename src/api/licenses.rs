//! License discovery and signed download URLs.

use std::path::{Path, PathBuf};

use super::{ApiClient, License, PagedLicenses};
use crate::error::Result;

impl ApiClient {
    pub async fn licenses(&self, customer_id: &str) -> Result<Vec<License>> {
        let mut page: PagedLicenses =
            self.get_json(&format!("/api/licenses/{customer_id}?page=1&limit=100")).await?;
        // Upstream reverses (end_date is None, end_date): null dates come first,
        // despite its comment saying "null dates last". Equal dates remain stable.
        page.items.sort_by(|a, b| match (&a.end_date, &b.end_date) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => b.cmp(a),
        });
        Ok(page.items)
    }

    pub async fn download_license(
        &self,
        customer_id: &str,
        license_key: &str,
        asset_type: &str,
        output: &Path,
    ) -> Result<Option<PathBuf>> {
        let url: Option<String> = self
            .get_json(&format!("/api/licenses/{customer_id}/download/{asset_type}/{license_key}"))
            .await?;
        match url.filter(|url| !url.is_empty()) {
            Some(url) => self.download_file(&url, output, None, false, false, None).await.map(Some),
            None => Ok(None),
        }
    }
}
