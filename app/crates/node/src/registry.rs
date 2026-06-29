//! HTTP client for the BitModel registry (manifests + seeder announces).

use anyhow::{Context, Result};
use bitmodel_core::Manifest;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct RegistryClient {
    base: String,
    http: reqwest::Client,
    token: Option<String>,
}

#[derive(Deserialize)]
struct SeedersResp {
    #[allow(dead_code)]
    model: String,
    seeders: Vec<String>,
}

impl RegistryClient {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into();
        let base = base.trim_end_matches('/').to_string();
        Self {
            base,
            http: reqwest::Client::new(),
            token: None,
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        let t = token.into();
        if !t.is_empty() {
            self.token = Some(t);
        }
        self
    }

    /// Fetch and parse the signed manifest for `model`.
    pub async fn get_manifest(&self, model: &str) -> Result<Manifest> {
        let url = format!("{}/manifest/{}", self.base, model);
        let resp = self.http.get(&url).send().await.context("GET manifest")?;
        anyhow::ensure!(
            resp.status().is_success(),
            "registry returned {} for {}",
            resp.status(),
            url
        );
        let text = resp.text().await?;
        Manifest::from_json(&text).context("parse manifest")
    }

    /// Publish a signed manifest (validator only; needs the token).
    pub async fn put_manifest(&self, model: &str, manifest: &Manifest) -> Result<()> {
        let url = format!("{}/manifest/{}", self.base, model);
        let mut req = self.http.put(&url).body(manifest.to_json()?);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let resp = req.send().await.context("PUT manifest")?;
        anyhow::ensure!(
            resp.status().is_success(),
            "publish failed: {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
        Ok(())
    }

    /// Announce (or heartbeat) that `node_id` seeds `model`.
    pub async fn announce(&self, model: &str, node_id: &str) -> Result<()> {
        let url = format!("{}/announce", self.base);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "model": model, "node_id": node_id }))
            .send()
            .await
            .context("POST announce")?;
        anyhow::ensure!(resp.status().is_success(), "announce failed: {}", resp.status());
        Ok(())
    }

    /// List live seeder NodeIds for `model`.
    pub async fn seeders(&self, model: &str) -> Result<Vec<String>> {
        let url = format!("{}/seeders/{}", self.base, model);
        let resp = self.http.get(&url).send().await.context("GET seeders")?;
        anyhow::ensure!(resp.status().is_success(), "seeders failed: {}", resp.status());
        let parsed: SeedersResp = resp.json().await.context("parse seeders")?;
        Ok(parsed.seeders)
    }
}
