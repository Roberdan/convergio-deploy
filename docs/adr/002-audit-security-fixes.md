# ADR-002: Security Audit Fixes

**Status**: Accepted
**Date**: 2025-07-21
**Author**: Security Audit (automated + manual review)

## Context

A comprehensive security audit of `convergio-deploy` identified several
vulnerabilities critical for a deployment system that manages binary upgrades
and fleet-wide push operations.

## Findings and Fixes

### CRITICAL

| # | Finding | Severity | Fix |
|---|---------|----------|-----|
| 1 | **SSRF via peer URLs** — `push_all` accepted arbitrary URLs, allowing requests to internal services (169.254.x, 10.x, localhost) | Critical | Added `validation::validate_peer_url()` with HTTPS-only, blocklist of RFC 1918/cloud metadata ranges |
| 2 | **Path traversal in tar extraction** — `extract_and_replace` used `std::env::temp_dir()` and had no post-extraction path validation | Critical | Moved staging to `convergio_data_dir()/upgrade-staging`, added `canonicalize()` + `starts_with()` check |
| 3 | **No input validation on version tags** — arbitrary strings could inject into GitHub API URLs | High | Added `validate_version_tag()` allowing only `[a-zA-Z0-9._-+]` |
| 4 | **Empty auth token accepted** — `unwrap_or_default()` allowed push-all with no authentication | High | Routes now reject push-all when `CONVERGIO_AUTH_TOKEN` is empty |

### MEDIUM

| # | Finding | Severity | Fix |
|---|---------|----------|-----|
| 5 | **Status endpoint leaked backup_path** — filesystem paths with usernames | Medium | Removed `last_upgrade` from status response |
| 6 | **Unbounded peer response in error messages** — log injection risk | Medium | Truncated peer error responses to 500 chars |
| 7 | **Defense-in-depth: double validation** — peer URLs validated at both route and trigger level | Low | Added `validate_peer_url()` call inside `trigger_peer_upgrade()` |

## Validation Module

New `src/validation.rs` provides:
- `validate_version_tag(tag)` — alphanumeric + `.` `-` `_` `+`, max 64 chars
- `validate_peer_url(url)` — HTTPS-only, blocks internal/metadata IPs
- `validate_peers(list)` — max 100 peers, each individually validated
- Comprehensive unit tests for all validators

## Decisions

1. **HTTPS-only for peers** — plaintext HTTP allows MITM on upgrade payloads
2. **Blocklist approach for SSRF** — pragmatic for embedded use; DNS rebinding
   remains a theoretical risk but is mitigated by HTTPS certificate validation
3. **Staging directory moved from /tmp** — `/tmp` is world-writable and subject to
   symlink attacks; using `convergio_data_dir()` is safer
4. **Checksum verification remains optional** — some releases may not have checksums;
   warning is logged but upgrade proceeds (supply chain risk acknowledged)

## Testing

- 9 unit tests added for input validation
- All existing tests continue to pass
- `cargo clippy` zero warnings
- `cargo fmt` clean
