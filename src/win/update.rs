use std::fs;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use tracing::{error, info};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::config;
use crate::core::install;
use crate::core::update::{
    self, assets_ready, parse_release, parse_releases, pick_release, Channel,
};

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("slate-update-check")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|err| format!("http client: {err}"))
}

fn marker_path() -> PathBuf {
    config::paths::data_dir().join(install::UPDATE_MARKER)
}

/// Records that a swapped-in build has not proven itself yet, so the next start can roll back.
fn mark_pending(version: &str) {
    let marker = serde_json::json!({ "version": version, "attempts": 0 });
    let path = marker_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(err) = fs::write(&path, marker.to_string()) {
        error!("could not write update marker: {err}");
    }
}

/// Clears the marker and drops the pre-update copy; only for a build that proved itself.
pub fn clear_update_marker() {
    clear_marker_only();
    // The new build reached its first frame, so the previous copy is no longer needed.
    let canonical = config::canonical_exe_path();
    let _ = fs::remove_file(install::backup_path(&canonical));
}

/// Clears the marker but keeps the backup, so a failed rollback can still be retried.
fn clear_marker_only() {
    let path = marker_path();
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

fn read_marker_attempts() -> Option<u32> {
    let text = fs::read_to_string(marker_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(value.get("attempts").and_then(|a| a.as_u64()).unwrap_or(0) as u32)
}

fn write_marker_attempts(attempts: u32) {
    let path = marker_path();
    let version = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| {
            value
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| crate::core::update::CURRENT_VERSION.to_string());
    let marker = serde_json::json!({ "version": version, "attempts": attempts });
    let _ = fs::write(&path, marker.to_string());
}

/// Runs before any UI work: a build that never reached its first frame is replaced by the backup.
pub fn recover_interrupted_update() -> Option<i32> {
    match install::launch_guard(read_marker_attempts()) {
        install::LaunchGuard::Clear => None,
        install::LaunchGuard::FirstStart => {
            write_marker_attempts(1);
            None
        }
        install::LaunchGuard::Rollback => {
            info!("the last update never started; rolling back");
            let canonical = config::canonical_exe_path();
            let backup = install::backup_path(&canonical);
            if !backup.exists() {
                error!("no backup to roll back to; keeping the current build");
                clear_marker_only();
                return None;
            }
            let failed = canonical.with_file_name(install::FAILED_NAME);
            let _ = fs::remove_file(&failed);
            if let Err(err) = fs::rename(&canonical, &failed) {
                error!("could not set the failed build aside: {err}");
                clear_marker_only();
                return None;
            }
            if let Err(err) = fs::rename(&backup, &canonical) {
                error!("could not put the backup back: {err}");
                let _ = fs::rename(&failed, &canonical);
                clear_marker_only();
                return None;
            }
            clear_update_marker();
            info!("rolled back to the previous build, relaunching");
            if let Err(err) = Command::new(&canonical)
                .creation_flags(CREATE_NO_WINDOW.0)
                .spawn()
            {
                error!("could not relaunch the restored build: {err}");
            }
            Some(0)
        }
    }
}

pub fn bootstrap(source: &str) -> Result<(), String> {
    info!("bootstrap update from {source}");
    let source_path = Path::new(source);
    let (version, arch) = install::parse_release_filename(source_path)
        .ok_or_else(|| "downloaded exe is not a recognized release asset".to_string())?;
    let canonical = config::canonical_exe_path();
    if let Some(parent) = canonical.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("slate dir: {err}"))?;
    }
    let staging = config::staging_updates_dir();
    fs::create_dir_all(&staging).map_err(|err| format!("staging dir: {err}"))?;
    let staged = staging.join(install::STAGED_NAME);
    fs::copy(source_path, &staged).map_err(|err| format!("stage: {err}"))?;
    let asset = update::asset_name(&version, &arch);
    let sums = fetch_sums(&version)?;
    let expected = install::checksum_for(&sums, &asset)
        .ok_or_else(|| format!("missing checksum for {asset} in SHA256SUMS"))?;
    let actual = file_hash(&staged)?;
    if actual != expected {
        let _ = fs::remove_file(&staged);
        error!("checksum mismatch for {asset}");
        return Err(format!(
            "checksum mismatch for {asset}: expected {expected}, got {actual}"
        ));
    }
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

pub fn refresh_install_from(candidate: &str) -> Result<(), String> {
    info!("refresh install from {candidate}");
    let candidate_path = Path::new(candidate);
    let (version, _arch) = install::parse_release_filename(candidate_path)
        .ok_or_else(|| "downloaded exe is not a recognized release asset".to_string())?;
    let canonical = config::canonical_exe_path();
    let downloaded = download_release(&version)?;
    let pid = std::process::id();
    mark_pending(&version);
    spawn_swap_helper(pid, &canonical, &downloaded)?;
    if !config::save_install(&config::InstallRecord {
        path: canonical.to_string_lossy().into_owned(),
        version: version.clone(),
        pid,
    }) {
        return Err("failed to write install record".to_string());
    }
    info!("swap helper spawned for version {version}");
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

pub fn download_release(version: &str) -> Result<PathBuf, String> {
    download_release_with_progress(version, |_| {})
}

/// Same as `download_release`, but reports how much of the asset has arrived, 0 to 100.
pub fn download_release_with_progress(
    version: &str,
    mut progress: impl FnMut(u8),
) -> Result<PathBuf, String> {
    info!("downloading release v{version}");
    let arch = asset_arch();
    let asset = update::asset_name(version, arch);
    let url = update::download_url(version, arch);
    let staging = config::staging_updates_dir();
    fs::create_dir_all(&staging).map_err(|err| format!("staging dir: {err}"))?;
    let dest = staging.join(install::staged_download_name(version, arch));
    let client = http_client()?;
    let mut res = client
        .get(&url)
        .header("User-Agent", "slate-updater")
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
    if res.status().as_u16() == 404 {
        let _ = fs::remove_file(&dest);
        return Err("Release asset not ready yet".to_string());
    }
    if !res.status().is_success() {
        let _ = fs::remove_file(&dest);
        return Err(format!("Network error {}", res.status().as_u16()));
    }
    let total = res.content_length().unwrap_or(0);
    let mut file = fs::File::create(&dest).map_err(|err| format!("create file: {err}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    let mut reported: u8 = u8::MAX;
    loop {
        let read = match res.read(&mut buf) {
            Ok(read) => read,
            Err(err) => {
                let _ = fs::remove_file(&dest);
                return Err(format!("download: {err}"));
            }
        };
        if read == 0 {
            break;
        }
        if let Err(err) = file.write_all(&buf[..read]) {
            let _ = fs::remove_file(&dest);
            return Err(format!("write file: {err}"));
        }
        received += read as u64;
        if let Some(percent) = (received.saturating_mul(100))
            .checked_div(total)
            .map(|value| value.min(100) as u8)
        {
            if percent != reported {
                reported = percent;
                progress(percent);
            }
        }
    }
    progress(100);
    let _ = file.flush();
    let sums = match fetch_sums(version) {
        Ok(sums) => sums,
        Err(err) => {
            let _ = fs::remove_file(&dest);
            return Err(err);
        }
    };
    let expected = match install::checksum_for(&sums, &asset) {
        Some(expected) => expected,
        None => {
            let _ = fs::remove_file(&dest);
            return Err(format!("missing checksum for {asset} in SHA256SUMS"));
        }
    };
    let actual = file_hash(&dest)?;
    if actual != expected {
        let _ = fs::remove_file(&dest);
        error!("checksum mismatch for {asset}");
        return Err(format!(
            "checksum mismatch for {asset}: expected {expected}, got {actual}"
        ));
    }
    info!("downloaded and verified {asset}");
    Ok(dest)
}

fn fetch_sums(version: &str) -> Result<String, String> {
    let sums_url = format!(
        "https://github.com/{}/releases/download/v{version}/SHA256SUMS",
        update::REPO_SLUG
    );
    let client = http_client()?;
    let res = client
        .get(&sums_url)
        .header("User-Agent", "slate-updater")
        .send()
        .map_err(|err| format!("fetch sums: {err}"))?;
    if !res.status().is_success() {
        return Err(format!(
            "failed to fetch SHA256SUMS: {}",
            res.status().as_u16()
        ));
    }
    let sums = res.text().map_err(|err| format!("read sums: {err}"))?;
    Ok(sums)
}

fn file_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|err| format!("open file: {err}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| format!("read file: {err}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

fn cmd_exe() -> std::path::PathBuf {
    let shell = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("System32")
        .join("cmd.exe");
    if shell.is_file() {
        return shell;
    }
    std::env::var_os("ComSpec")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("cmd.exe"))
}

pub fn spawn_swap_helper(pid: u32, target: &Path, staged: &Path) -> Result<(), String> {
    let dir = config::staging_updates_dir();
    let name = swap_helper_name(pid);
    std::fs::write(dir.join(&name), helper_script(pid, target, staged))
        .map_err(|err| format!("helper: {err}"))?;
    Command::new(cmd_exe())
        .current_dir(&dir)
        .args(["/c"])
        .arg(&name)
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()
        .map_err(|err| format!("helper: {err}"))?;
    Ok(())
}

fn swap_helper_name(pid: u32) -> String {
    format!("slate-swap-{pid}.bat")
}

fn batch_arg(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy().replace('%', "%%"))
}

fn helper_script(pid: u32, target: &Path, staged: &Path) -> String {
    let backup = install::backup_path(target);
    let staged = batch_arg(staged);
    let target = batch_arg(target);
    let backup = batch_arg(&backup);
    let tries = crate::timings::SWAP_WAIT_SECS;
    format!(
        "@echo off\r\n\
         setlocal DisableDelayedExpansion\r\n\
         set /a n=0\r\n\
         :wait\r\n\
         tasklist /fi \"PID eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
         if errorlevel 1 goto swap\r\n\
         set /a n+=1\r\n\
         if %n% geq {tries} goto swap\r\n\
         timeout /t 1 /nobreak >nul 2>&1 || ping -n 2 127.0.0.1 >nul\r\n\
         goto wait\r\n\
         :swap\r\n\
         if not exist {staged} goto launch\r\n\
         if not exist {target} goto replace\r\n\
         if exist {backup} goto replace\r\n\
         move /y {target} {backup} >nul\r\n\
         :replace\r\n\
         move /y {staged} {target} >nul\r\n\
         if exist {target} goto launch\r\n\
         if exist {backup} move /y {backup} {target} >nul\r\n\
         :launch\r\n\
         if exist {target} start \"\" {target}\r\n\
         if not exist {target} if exist {staged} start \"\" {staged}\r\n\
         del \"%~f0\" >nul\r\n",
        pid = pid,
        staged = staged,
        target = target,
        backup = backup,
        tries = tries,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = "C:\\Users\\a\\.burntfern\\slate\\slate.exe";
    const STAGED: &str = "C:\\Users\\a\\.burntfern\\slate\\updates\\slate-update-v0.2.0-x64.exe";

    #[test]
    fn swap_helper_waits_then_replaces_and_relaunches() {
        let script = helper_script(
            4242,
            Path::new("C:\\apps\\slate.exe"),
            Path::new("C:\\Temp\\slate-update.exe"),
        );
        assert!(script.contains("tasklist /fi \"PID eq 4242\""));
        assert!(script.contains("if errorlevel 1 goto swap"));
        assert!(script.contains("start \"\" \"C:\\apps\\slate.exe\""));
        assert!(script.contains("del \"%~f0\""));
        assert!(script.contains(&format!(
            "if %n% geq {} goto swap",
            crate::timings::SWAP_WAIT_SECS
        )));
        assert!(script.contains("timeout /t 1 /nobreak"));
    }

    #[test]
    fn rate_limit_waits_for_the_reset_header() {
        assert!(rate_limit_message(Some(1_000 + 1_200), 1_000).contains("20 min"));
        assert!(rate_limit_message(Some(1_000 + 30), 1_000).contains("shortly"));
        assert!(rate_limit_message(Some(900), 1_000).contains("shortly"));
        assert!(rate_limit_message(None, 1_000).contains("60/hour"));
    }

    #[test]
    fn swap_helper_backs_up_and_swaps_without_reverifying() {
        let target = Path::new(TARGET);
        let staged = Path::new(STAGED);
        let backup = install::backup_path(target);
        let script = helper_script(7, target, staged);

        assert!(!script.contains("certutil"));
        assert!(!script.contains("slate-new.exe"));
        assert!(script.contains("setlocal DisableDelayedExpansion"));

        assert!(script.contains(&format!("if not exist {} goto replace", batch_arg(target))));
        assert!(script.contains(&format!("if exist {} goto replace", batch_arg(&backup))));

        let back_up = format!("move /y {} {} >nul", batch_arg(target), batch_arg(&backup));
        let swap = format!("move /y {} {} >nul", batch_arg(staged), batch_arg(target));
        let restore = format!("move /y {} {} >nul", batch_arg(&backup), batch_arg(target));
        assert!(script.contains(&back_up));
        assert!(script.contains(&swap));
        assert!(script.contains(&restore));
        assert!(script.find(&back_up).unwrap() < script.find(&swap).unwrap());
    }

    #[test]
    fn batch_paths_stay_literal_for_cmd() {
        assert_eq!(
            batch_arg(Path::new("C:\\Users\\John Doe\\slate.exe")),
            "\"C:\\Users\\John Doe\\slate.exe\""
        );
        assert_eq!(
            batch_arg(Path::new("C:\\100%\\slate.exe")),
            "\"C:\\100%%\\slate.exe\""
        );
        assert_eq!(
            batch_arg(Path::new("C:\\a&b^c|d(e)\\slate.exe")),
            "\"C:\\a&b^c|d(e)\\slate.exe\""
        );
    }

    #[test]
    fn helper_script_escapes_special_paths() {
        let target = Path::new("C:\\Users\\a%b&c\\.burntfern\\slate\\slate.exe");
        let staged =
            Path::new("C:\\Users\\a%b&c\\.burntfern\\slate\\updates\\slate-update-v0.2.0-x64.exe");
        let script = helper_script(7, target, staged);

        assert!(!script.contains("a%b"));
        assert!(script.contains("\"C:\\Users\\a%%b&c\\.burntfern\\slate\\slate.exe\""));
        assert!(script.contains("\"C:\\Users\\a%%b&c\\.burntfern\\slate\\slate.exe.old\""));
        assert!(script.contains("\"C:\\Users\\a%%b&c\\.burntfern\\slate\\updates\\"));
    }

    #[test]
    fn swap_helper_is_handed_a_bare_name() {
        let name = swap_helper_name(4242);
        assert_eq!(name, "slate-swap-4242.bat");
        assert!(name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.')));
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
