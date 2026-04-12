//! GitHub Release API client.

use crate::types::{Release, ReleaseAsset};
use reqwest::Client;

const REPO_OWNER: &str = "Roberdan";
const REPO_NAME: &str = "convergio";

/// Fetch the latest release from GitHub.
pub async fn fetch_latest_release() -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");
    fetch_release(&url).await
}

/// Fetch a specific release by tag.
pub async fn fetch_release_by_tag(tag: &str) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/tags/{tag}");
    fetch_release(&url).await
}

async fn fetch_release(url: &str) -> Result<Release, String> {
    let client = Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "convergio-deploy")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let tag = json["tag_name"].as_str().unwrap_or("unknown").to_string();
    let published = json["published_at"].as_str().unwrap_or("").to_string();

    let assets = json["assets"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| ReleaseAsset {
                    name: a["name"].as_str().unwrap_or("").to_string(),
                    url: a["browser_download_url"].as_str().unwrap_or("").to_string(),
                    size: a["size"].as_u64().unwrap_or(0),
                    sha256: None,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Release {
        tag,
        assets,
        published_at: published,
    })
}

/// Find the asset matching the current platform.
pub fn find_platform_asset<'a>(release: &'a Release, platform: &str) -> Option<&'a ReleaseAsset> {
    release.assets.iter().find(|a| a.name.contains(platform))
}

/// Download asset bytes and return them.
pub async fn download_asset(asset: &ReleaseAsset) -> Result<Vec<u8>, String> {
    let client = Client::new();
    let resp = client
        .get(&asset.url)
        .header("User-Agent", "convergio-deploy")
        .send()
        .await
        .map_err(|e| format!("Download error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download HTTP {}", resp.status()));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Read error: {e}"))
}

/// Download the SHA256 checksum file for a release.
pub async fn fetch_checksums(release: &Release) -> Result<String, String> {
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name.contains("SHA256") || a.name.ends_with(".sha256"))
        .ok_or("No checksum file in release")?;

    let bytes = download_asset(checksum_asset).await?;
    String::from_utf8(bytes).map_err(|e| format!("Checksum not UTF-8: {e}"))
}
