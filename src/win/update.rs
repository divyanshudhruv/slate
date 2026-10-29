use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use reqwest::blocking::Client;
use tracing::{error, info};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::config;
use crate::core::install;
use crate::core::update::{assets_ready, parse_release, parse_releases, pick_release, Channel};

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("slate-update-check")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|err| format!("http client: {err}"))
}

/// Drops the pre-update copy once a published build has reached its first frame.
pub fn drop_update_backup() {
    let canonical = config::canonical_exe_path();
    let _ = fs::remove_file(install::backup_path(&canonical));
}

/// Copies the running release into the install dir and publishes it.
/// The copy is already executing, so there is nothing left to verify here.
pub fn bootstrap(source: &str) -> Result<(), String> {
    info!("bootstrap update from {source}");
    let source_path = Path::new(source);
    let (version, _arch) = install::parse_release_filename(source_path)
        .ok_or_else(|| "downloaded exe is not a recognized release asset".to_string())?;
    let canonical = config::canonical_exe_path();
    if let Some(parent) = canonical.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("slate dir: {err}"))?;
    }
    let staging = config::staging_updates_dir();
    fs::create_dir_all(&staging).map_err(|err| format!("staging dir: {err}"))?;
    let staged = staging.join(install::STAGED_NAME);
    fs::copy(source_path, &staged).map_err(|err| format!("stage: {err}"))?;
    let backup = install::backup_path(&canonical);
    let had_backup = backup.exists();
    if install::needs_backup(canonical.exists(), had_backup) {
        if let Err(err) = fs::rename(&canonical, &backup) {
            return Err(format!("backup: {err}"));
        }
    }
    if let Err(err) = fs::rename(&staged, &canonical) {
        restore_backup(&backup, &canonical);
        return Err(format!("publish: {err}"));
    }
    if !config::save_install(&config::InstallRecord {
        path: canonical.to_string_lossy().into_owned(),
        version: version.clone(),
        pid: std::process::id(),
    }) {
        restore_backup(&backup, &canonical);
        return Err("failed to write install record".to_string());
    }
    info!("bootstrap complete version {version}");
    Ok(())
}

/// Puts the previous exe back when a publish step fails midway.
fn restore_backup(backup: &Path, canonical: &Path) {
    if !backup.exists() {
        return;
    }
    let _ = fs::remove_file(canonical);
    if let Err(err) = fs::rename(backup, canonical) {
        error!("rollback failed: {err}");
    }
}

/// Copies a pre-0.5 install into the install dir, then relaunches it there.
pub fn relocate(source: &str) -> Result<(), String> {
    info!("relocating install from {source}");
    let canonical = config::canonical_exe_path();
    let Some(parent) = canonical.parent() else {
        return Err("install dir has no parent".to_string());
    };
    fs::create_dir_all(parent).map_err(|err| format!("install dir: {err}"))?;
    if !canonical.exists() {
        fs::copy(source, &canonical).map_err(|err| format!("copy exe: {err}"))?;
    }
    if !config::save_install(&config::InstallRecord {
        path: canonical.to_string_lossy().into_owned(),
        version: crate::core::update::CURRENT_VERSION.to_string(),
        pid: 0,
    }) {
        return Err("failed to write install record".to_string());
    }
    Command::new(&canonical)
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()
        .map_err(|err| format!("launch: {err}"))?;
    info!("relaunched from {}", canonical.display());
    Ok(())
}

pub fn fetch_latest(channel: Channel) -> Result<String, String> {
    let client = http_client()?;
    let res = client
        .get(channel.api_url())
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "slate-update-check")
        .send()
        .map_err(|err| {
            if err.is_timeout() {
                "Timed out - try again".to_string()
            } else if err.is_connect() {
                "Offline - could not reach github.com".to_string()
            } else {
                format!("Network error: {err}")
            }
        })?;
    let status = res.status();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        let reset = res
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok());
        return Err(rate_limit_message(reset, chrono_now()));
    }
    if status.as_u16() == 404 {
        return Err("No releases published yet".to_string());
    }
    if !status.is_success() {
        return Err(format!("GitHub returned HTTP {}", status.as_u16()));
    }
    let body = res.text().map_err(|err| format!("read body: {err}"))?;
    let release = match channel {
        Channel::Stable => {
            parse_release(&body).ok_or_else(|| "no release tag in response".to_string())?
        }
        Channel::Beta => {
            return pick_release(parse_releases(&body), asset_arch())
                .map(|release| release.version)
                .ok_or_else(|| "No beta release with downloads yet".to_string())
        }
    };
    // release-please publishes the release before the build workflow uploads assets, so a bare tag must not count as an installable update.
    if !assets_ready(&release, asset_arch()) {
        return Err(format!(
            "v{} is out, but its download is not uploaded yet - try again soon",
            release.version
        ));
    }
    Ok(release.version)
}

/// Turns GitHub's reset timestamp into a wait the user can act on.
fn rate_limit_message(reset: Option<i64>, now: i64) -> String {
    match reset.map(|at| (at - now) / 60) {
        Some(wait) if wait > 0 => {
            format!("GitHub is rate-limiting update checks - try again in about {wait} min")
        }
        Some(_) => "GitHub is rate-limiting update checks - try again shortly".to_string(),
        None => "GitHub is rate-limiting update checks (60/hour) - try later".to_string(),
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn asset_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_waits_for_the_reset_header() {
        assert!(rate_limit_message(Some(1_000 + 1_200), 1_000).contains("20 min"));
        assert!(rate_limit_message(Some(1_000 + 30), 1_000).contains("shortly"));
        assert!(rate_limit_message(Some(900), 1_000).contains("shortly"));
        assert!(rate_limit_message(None, 1_000).contains("60/hour"));
    }

    #[test]
    fn publish_path_is_the_canonical_slate_exe() {
        let target = config::canonical_exe_path();
        assert!(target
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("slate.exe")));
    }
}
