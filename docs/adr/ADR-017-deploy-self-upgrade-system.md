---
version: "1.0"
last_updated: "2026-04-07"
author: "convergio-team"
tags: ["adr"]
---

# ADR-017: Deploy & Self-Upgrade System

**Status**: Accepted  
**Date**: 2026-04-05  
**Context**: Session 10 — Deploy system

## Decision

Create `convergio-deploy` crate providing:
1. **Self-upgrade**: download from GitHub Releases, verify SHA256, backup, replace, restart
2. **Push-all**: upgrade all mesh peers (rolling or parallel)
3. **Auto-update check**: scheduled task every 6h checks GitHub for new versions, notifies user
4. **Diagnostics**: self-test suite with auto-GitHub-issue creation (no sensitive data)

### Upgrade Flow

1. Fetch release from GitHub API
2. Find asset matching current platform triple
3. Download and verify SHA256 checksum
4. Backup current binary to `{data_dir}/backups/`
5. Extract and replace binary
6. Restart via platform service manager (launchd/systemd/Windows service)
7. On failure: automatic rollback from backup

### Update Notification

Daemon checks for updates every 6 hours. If a newer version exists:
- Logs info message with version
- Stores notification in DB for cockpit display
- CLI `cvg deploy status` shows available update
- Tauri app shows update banner
- Does NOT auto-upgrade unless configured

### Diagnostics

`POST /api/deploy/diagnostics` runs 7 checks: database, health, routes, disk, config, GitHub API, deploy status.
`POST /api/deploy/diagnostics/report-issue` creates a GitHub issue with sanitized report (strips home paths, tokens).

## Rationale

- Self-upgrade eliminates manual SSH-and-copy workflow for multi-node mesh
- SHA256 verification prevents corrupted/tampered binaries
- Backup+rollback prevents bricked nodes
- Diagnostics with auto-issue creation reduces support burden

## Platform-Specific Notes

- **macOS**: restart via `launchctl unload/load`, binary at `/usr/local/bin/` or alongside app
- **Linux**: restart via `systemctl --user restart convergio`
- **Windows**: restart via `net stop/start Convergio` (manual NSSM setup for now)
- **MLX inference**: macOS Apple Silicon only (M1/M2/M3/M4)
- **Keychain storage**: macOS only (Telegram bot token)

## Consequences

- New crate: `convergio-deploy` (33rd crate in workspace)
- 3 new DB tables: `deploy_upgrades`, `deploy_push_jobs`, `deploy_notifications`
- 6 new HTTP endpoints under `/api/deploy/`
- CLI: `cvg deploy {upgrade,push-all,status,history}`
