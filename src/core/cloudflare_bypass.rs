//! Cloudflare bypass stub - disabled in demo build (no auth required).

use anyhow::{anyhow, Result};

#[derive(Clone, Debug)]
pub enum BypassResult {
    Success { cf_clearance: String, content: String },
    Cancelled,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum BrowserStatus {
    Launching,
    WaitingForVerification,
    Verified,
    DownloadingContent,
}

pub struct CookieCache {
    pub cf_clearance: String,
}

/// Always returns None - no cached cookies in demo build.
pub fn load_cached_cookie(_domain: &str) -> Option<CookieCache> {
    None
}

/// Always returns an error - no browser bypass in demo build.
pub async fn download_with_cloudflare_bypass(_url: &str) -> Result<String> {
    Err(anyhow!("Cloudflare bypass not available in demo build"))
}

pub struct CloudflareBypasser;

impl CloudflareBypasser {
    pub fn new() -> Self { Self }
}
