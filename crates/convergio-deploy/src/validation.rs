//! Input validation for deploy endpoints.
//! Prevents SSRF, injection, and malformed input.

/// Maximum version string length.
const MAX_VERSION_LEN: usize = 64;

/// Maximum peer URL length.
const MAX_PEER_URL_LEN: usize = 2048;

/// Maximum number of peers in a push-all request.
pub const MAX_PEERS: usize = 100;

/// Validate a semver-like version or git tag (e.g. "v1.2.3", "1.2.3-rc.1").
/// Rejects characters that could enable URL injection.
pub fn validate_version_tag(tag: &str) -> Result<(), String> {
    if tag.is_empty() {
        return Err("Version tag cannot be empty".into());
    }
    if tag.len() > MAX_VERSION_LEN {
        return Err(format!(
            "Version tag too long ({} > {MAX_VERSION_LEN})",
            tag.len()
        ));
    }
    // Allow: alphanumeric, dots, hyphens, underscores, leading 'v'
    if !tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
    {
        return Err(format!("Version tag contains invalid characters: {tag}"));
    }
    Ok(())
}

/// Validate a peer URL to prevent SSRF attacks.
/// Only allows HTTPS URLs to non-internal hosts.
pub fn validate_peer_url(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("Peer URL cannot be empty".into());
    }
    if url.len() > MAX_PEER_URL_LEN {
        return Err("Peer URL too long".into());
    }

    // Must be HTTPS (prevent plaintext MITM)
    if !url.starts_with("https://") {
        return Err(format!("Peer URL must use HTTPS: {url}"));
    }

    // Extract host portion
    let host = url
        .strip_prefix("https://")
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next())
        .unwrap_or("");

    if host.is_empty() {
        return Err("Peer URL has no host".into());
    }

    // Block internal/metadata IPs (SSRF protection)
    if is_internal_host(host) {
        return Err(format!(
            "Peer URL points to internal/reserved address: {host}"
        ));
    }

    Ok(())
}

/// Check if a host resolves to an internal/reserved IP range.
fn is_internal_host(host: &str) -> bool {
    // Block well-known internal hostnames
    let blocked = [
        "localhost",
        "127.0.0.1",
        "0.0.0.0",
        "[::1]",
        "metadata.google.internal",
    ];
    if blocked.iter().any(|b| host.eq_ignore_ascii_case(b)) {
        return true;
    }

    // Block cloud metadata IPs
    let metadata_prefixes = [
        "169.254.", // AWS/Azure metadata
        "10.",      // RFC 1918
        "172.16.",  // RFC 1918
        "172.17.", "172.18.", "172.19.", "172.20.", "172.21.", "172.22.", "172.23.", "172.24.",
        "172.25.", "172.26.", "172.27.", "172.28.", "172.29.", "172.30.", "172.31.",
        "192.168.", // RFC 1918
    ];
    metadata_prefixes.iter().any(|p| host.starts_with(p))
}

/// Validate a list of peer URLs.
pub fn validate_peers(peers: &[String]) -> Result<(), String> {
    if peers.is_empty() {
        return Err("Peers list cannot be empty".into());
    }
    if peers.len() > MAX_PEERS {
        return Err(format!("Too many peers ({} > {MAX_PEERS})", peers.len()));
    }
    for (i, url) in peers.iter().enumerate() {
        validate_peer_url(url).map_err(|e| format!("peers[{i}]: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_version_tags() {
        assert!(validate_version_tag("v1.2.3").is_ok());
        assert!(validate_version_tag("1.0.0").is_ok());
        assert!(validate_version_tag("1.0.0-rc.1").is_ok());
        assert!(validate_version_tag("v0.1.0-beta+build.123").is_ok());
    }

    #[test]
    fn invalid_version_tags() {
        assert!(validate_version_tag("").is_err());
        assert!(validate_version_tag("v1.2.3/../../etc").is_err());
        assert!(validate_version_tag("v1; rm -rf /").is_err());
        assert!(validate_version_tag("v1\n\rinjection").is_err());
        let long = "v".repeat(100);
        assert!(validate_version_tag(&long).is_err());
    }

    #[test]
    fn valid_peer_urls() {
        assert!(validate_peer_url("https://node1.example.com:8420").is_ok());
        assert!(validate_peer_url("https://deploy.convergio.io").is_ok());
    }

    #[test]
    fn ssrf_blocked() {
        assert!(validate_peer_url("http://node1.example.com").is_err());
        assert!(validate_peer_url("https://127.0.0.1").is_err());
        assert!(validate_peer_url("https://localhost").is_err());
        assert!(validate_peer_url("https://169.254.169.254").is_err());
        assert!(validate_peer_url("https://10.0.0.1").is_err());
        assert!(validate_peer_url("https://192.168.1.1").is_err());
        assert!(validate_peer_url("https://metadata.google.internal").is_err());
    }

    #[test]
    fn peers_limits() {
        assert!(validate_peers(&[]).is_err());
        let many: Vec<String> = (0..101)
            .map(|i| format!("https://node{i}.example.com"))
            .collect();
        assert!(validate_peers(&many).is_err());
    }
}
