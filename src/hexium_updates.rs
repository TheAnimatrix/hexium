//! Hexium: a daily look at Hexium's newest GitHub release. Hex's own updater
//! stays off (it would install Hex); when a release built from a different
//! commit is out, the sidebar offers "Download Hexium update", which opens its
//! release page. Nothing installs by itself, and nothing about the person or
//! the computer is sent: one anonymous GET to the GitHub API a day.
//! `HEXIUM_NO_UPDATE_CHECK=1` turns it off.

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use color_eyre::eyre::{Result, eyre};

const LATEST: &str = "https://api.github.com/repos/TheAnimatrix/hexium/releases/latest";
/// The commit this build came from (`build.rs`); releases are tagged
/// `v<version>-hexium.<commit>`.
const BUILD_COMMIT: Option<&str> = option_env!("HEXIUM_COMMIT");
const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_BYTES: u64 = 256 * 1024;

/// A newer Hexium release.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

static FOUND: Mutex<Option<Release>> = Mutex::new(None);
static STARTED: OnceLock<()> = OnceLock::new();

/// Starts the daily check once per process.
pub fn start() {
    if std::env::var_os("HEXIUM_NO_UPDATE_CHECK").is_some() {
        return;
    }
    let Some(commit) = BUILD_COMMIT.filter(|commit| !commit.is_empty()) else {
        return;
    };
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("hexium-updates".into())
            .spawn(move || {
                std::thread::sleep(FIRST_CHECK);
                loop {
                    match check(commit) {
                        Ok(found) => *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = found,
                        Err(error) => tracing::info!(%error, "Hexium update check failed"),
                    }
                    std::thread::sleep(EVERY);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not start the Hexium update check");
        }
    });
}

/// The newer release the last check found, if any.
pub fn available() -> Option<Release> {
    FOUND.lock().ok()?.clone()
}

/// Opens a release page in the browser.
pub fn open(url: &str) {
    if let Err(error) = crate::desktop::opencode_catalog::open_url(url) {
        tracing::error!(%error, "could not open the Hexium release page");
    }
}

fn check(commit: &str) -> Result<Option<Release>> {
    let agent = crate::net::agent(LATEST, |config| {
        config
            .https_only(true)
            .max_redirects(5)
            .timeout_global(Some(Duration::from_secs(20)))
    });
    let mut response = agent
        .get(LATEST)
        .header("User-Agent", "Hexium")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| eyre!("{}", crate::net::unreachable(LATEST, &error)))?;
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_string()?;
    newer(&body, commit)
}

/// The release in a GitHub `releases/latest` reply, unless it was built from
/// `commit`.
fn newer(body: &str, commit: &str) -> Result<Option<Release>> {
    let value: serde_json::Value = serde_json::from_str(body)?;
    let tag = value["tag_name"]
        .as_str()
        .ok_or_else(|| eyre!("the release has no tag"))?;
    let url = value["html_url"]
        .as_str()
        .filter(|url| url.starts_with("https://github.com/TheAnimatrix/hexium/"))
        .ok_or_else(|| eyre!("the release has no Hexium page"))?;
    let Some((version, built_from)) = tag.trim_start_matches('v').split_once("-hexium.") else {
        return Ok(None);
    };
    if built_from.starts_with(commit) || commit.starts_with(built_from) {
        return Ok(None);
    }
    Ok(Some(Release {
        version: version.to_owned(),
        url: url.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = r#"{"tag_name":"v3.4.0-hexium.abc1234","html_url":"https://github.com/TheAnimatrix/hexium/releases/tag/v3.4.0-hexium.abc1234"}"#;

    #[test]
    fn a_release_from_another_commit_is_newer() {
        assert_eq!(
            newer(REPLY, "56e5f5d").unwrap(),
            Some(Release {
                version: "3.4.0".into(),
                url: "https://github.com/TheAnimatrix/hexium/releases/tag/v3.4.0-hexium.abc1234"
                    .into(),
            })
        );
    }

    #[test]
    fn the_release_this_build_came_from_is_not() {
        assert_eq!(newer(REPLY, "abc1234").unwrap(), None);
    }

    #[test]
    fn a_page_outside_hexium_is_refused() {
        let reply = r#"{"tag_name":"v3.4.0-hexium.abc1234","html_url":"https://example.com/x"}"#;
        assert!(newer(reply, "56e5f5d").is_err());
    }
}
