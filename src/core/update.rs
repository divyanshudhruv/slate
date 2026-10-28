pub const REPO_SLUG: &str = "divyanshudhruv/slate";
pub const REPOSITORY_URL: &str = "https://github.com/divyanshudhruv/slate";
#[allow(dead_code)]
pub const RELEASES_URL: &str = "https://github.com/divyanshudhruv/slate/releases/latest";
pub const LATEST_API_URL: &str =
    "https://api.github.com/repos/divyanshudhruv/slate/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Five minutes between automatic checks; a manual check bypasses it.
pub const CHECK_COOLDOWN_SECS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed,
}

impl UpdateState {
    pub fn from_cached(current: &str, cached: Option<&str>) -> Self {
        match cached {
            Some(version) if is_newer(current, version) => Self::Available(version.to_string()),
            Some(_) => Self::UpToDate,
            None => Self::Idle,
        }
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let mut parts = text.split('.');
    let major = leading_number(parts.next()?)?;
    let minor = parts.next().and_then(leading_number).unwrap_or(0);
    let patch = parts.next().and_then(leading_number).unwrap_or(0);
    Some((major, minor, patch))
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

pub fn asset_name(tag: &str, arch: &str) -> String {
    let tag = tag.trim().trim_start_matches(['v', 'V']);
    format!("slate-v{tag}-{arch}.exe")
}

pub fn download_url(tag: &str, arch: &str) -> String {
    let tag = tag.trim().trim_start_matches(['v', 'V']);
    format!(
        "https://github.com/{REPO_SLUG}/releases/download/v{tag}/{}",
        asset_name(tag, arch)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub assets: Vec<String>,
}

/// Which release line update checks follow; beta sees prereleases first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }

    pub fn api_url(self) -> String {
        match self {
            Self::Stable => LATEST_API_URL.to_string(),
            Self::Beta => {
                format!("https://api.github.com/repos/{REPO_SLUG}/releases?per_page=20")
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for Channel {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(match text.to_ascii_lowercase().as_str() {
            "beta" => Self::Beta,
            "stable" => Self::Stable,
            other => {
                tracing::warn!("unknown update channel '{other}', using stable");
                Self::Stable
            }
        })
    }
}

fn release_from_value(value: &serde_json::Value) -> Option<Release> {
    let tag = value.get("tag_name")?.as_str()?.trim();
    let version = tag.trim_start_matches(['v', 'V']);
    if version.is_empty() {
        return None;
    }
    let assets = value
        .get("assets")
        .and_then(|assets| assets.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| asset.get("name").and_then(|name| name.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(Release {
        version: version.to_string(),
        assets,
    })
}

pub fn parse_release(body: &str) -> Option<Release> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    release_from_value(&value)
}

/// GitHub's release list, newest first, for the beta channel.
pub fn parse_releases(body: &str) -> Vec<Release> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(body) else {
        return Vec::new();
    };
    items.iter().filter_map(release_from_value).collect()
}

/// The newest listed release whose downloads are fully uploaded.
pub fn pick_release(releases: Vec<Release>, arch: &str) -> Option<Release> {
    releases
        .into_iter()
        .filter(|release| assets_ready(release, arch))
        .max_by(|a, b| {
            parse_version(&a.version)
                .unwrap_or_default()
                .cmp(&parse_version(&b.version).unwrap_or_default())
        })
}

pub fn assets_ready(release: &Release, arch: &str) -> bool {
    let exe = asset_name(&release.version, arch);
    release.assets.iter().any(|name| name == &exe)
        && release.assets.iter().any(|name| name == "SHA256SUMS")
}

fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_prefixed_versions() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("1"), Some((1, 0, 0)));
        assert_eq!(parse_version("0.1.0-beta"), Some((0, 1, 0)));
        assert_eq!(parse_version("nonsense"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn newer_compares_each_component() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "1.0.0"));
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "nonsense"));
    }

    #[test]
    fn cached_state_uses_version_compare() {
        assert_eq!(
            UpdateState::from_cached("0.1.0", Some("0.2.0")),
            UpdateState::Available("0.2.0".into())
        );
        assert_eq!(
            UpdateState::from_cached("0.1.0", Some("0.1.0")),
            UpdateState::UpToDate
        );
        assert_eq!(UpdateState::from_cached("0.1.0", None), UpdateState::Idle);
    }

    #[test]
    fn url_constants_cannot_drift_from_the_repo_slug() {
        assert_eq!(REPOSITORY_URL, format!("https://github.com/{REPO_SLUG}"));
        assert_eq!(RELEASES_URL, format!("{REPOSITORY_URL}/releases/latest"));
        assert_eq!(
            LATEST_API_URL,
            format!("https://api.github.com/repos/{REPO_SLUG}/releases/latest")
        );
    }

    #[test]
    fn download_url_matches_release_asset_names() {
        assert_eq!(
            download_url("0.2.0", "x64"),
            "https://github.com/divyanshudhruv/slate/releases/download/v0.2.0/slate-v0.2.0-x64.exe"
        );
        assert_eq!(
            download_url("v0.2.0", "arm64"),
            "https://github.com/divyanshudhruv/slate/releases/download/v0.2.0/slate-v0.2.0-arm64.exe"
        );
    }

    #[test]
    fn asset_name_is_the_checksum_key() {
        assert_eq!(asset_name("0.2.0", "x64"), "slate-v0.2.0-x64.exe");
        assert_eq!(asset_name("v0.2.0", "arm64"), "slate-v0.2.0-arm64.exe");
    }

    #[test]
    fn beta_channel_picks_newest_ready_release() {
        let body = r#"[
            {"tag_name":"v0.6.0-beta","assets":[{"name":"slate-v0.6.0-beta-x64.exe"}]},
            {"tag_name":"v0.5.0","assets":[{"name":"slate-v0.5.0-x64.exe"},{"name":"SHA256SUMS"}]},
            {"tag_name":"v0.4.0","assets":[{"name":"slate-v0.4.0-x64.exe"},{"name":"SHA256SUMS"}]}
        ]"#;
        let picked = pick_release(parse_releases(body), "x64").expect("a ready release");
        assert_eq!(picked.version, "0.5.0");
        assert_eq!(Channel::Stable.api_url(), LATEST_API_URL);
        assert_eq!(
            Channel::Beta.api_url(),
            format!("https://api.github.com/repos/{REPO_SLUG}/releases?per_page=20")
        );
    }

    #[test]
    fn beta_channel_accepts_a_ready_prerelease() {
        let body = r#"[{"tag_name":"v0.6.0-beta","assets":[{"name":"slate-v0.6.0-beta-x64.exe"},{"name":"SHA256SUMS"}]}]"#;
        let picked = pick_release(parse_releases(body), "x64").expect("ready prerelease");
        assert_eq!(picked.version, "0.6.0-beta");
    }

    #[test]
    fn unknown_channel_falls_back_to_stable() {
        let channel: Channel = serde_json::from_str(r#""nightly""#).unwrap();
        assert_eq!(channel, Channel::Stable);
    }

    #[test]
    fn release_json_yields_version_and_assets() {
        let body = r#"{"tag_name":"v0.2.0","assets":[{"name":"slate-v0.2.0-x64.exe"}]}"#;
        let release = parse_release(body).expect("release parses");
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.assets, vec!["slate-v0.2.0-x64.exe".to_string()]);
        assert_eq!(parse_release("{}"), None);
        assert_eq!(parse_release("not json"), None);
        assert_eq!(parse_release(r#"{"tag_name":""}"#), None);
    }

    #[test]
    fn assets_ready_needs_the_arch_exe_and_checksums() {
        let ready = Release {
            version: "0.2.0".into(),
            assets: vec![
                "slate-v0.2.0-x64.exe".into(),
                "slate-v0.2.0-arm64.exe".into(),
                "SHA256SUMS".into(),
            ],
        };
        assert!(assets_ready(&ready, "x64"));
        assert!(assets_ready(&ready, "arm64"));
        for missing in [
            vec!["slate-v0.2.0-x64.exe".to_string()],
            vec!["SHA256SUMS".to_string()],
            Vec::new(),
        ] {
            let partial = Release {
                version: "0.2.0".into(),
                assets: missing,
            };
            assert!(!assets_ready(&partial, "x64"));
        }
    }
}
