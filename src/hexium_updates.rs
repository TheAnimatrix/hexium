//! Hexium: a look at Hexium's newest GitHub release, 10 seconds after launch,
//! then hourly and on "Check for updates" (as Hex's own updater checks). Hex's
//! updater stays off (it would install Hex); when a release built from a
//! different commit is out, the sidebar offers "Download Hexium update", which
//! opens its release page. Nothing installs by itself, and nothing about the
//! person or the computer is sent: one anonymous GET to the GitHub API.
//! `HEXIUM_NO_UPDATE_CHECK=1` turns it off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use color_eyre::eyre::{Result, eyre};

const LATEST: &str = "https://api.github.com/repos/TheAnimatrix/hexium/releases/latest";
/// The commit this build came from (`build.rs`); releases are tagged
/// `v<version>-hexium.<commit>`.
const BUILD_COMMIT: Option<&str> = option_env!("HEXIUM_COMMIT");
const FIRST_CHECK: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(60 * 60);
/// After a failed check (offline), the next one comes sooner.
const RETRY: Duration = Duration::from_secs(10 * 60);
const MAX_BYTES: u64 = 256 * 1024;

/// A newer Hexium release.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

/// Where the check stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Not checked yet, or turned off.
    Idle,
    Checking,
    Current,
    Failed,
    Found(Release),
}

static STATUS: Mutex<Status> = Mutex::new(Status::Idle);
/// Set when [`STATUS`] changes, so the window redraws.
static CHANGED: AtomicBool = AtomicBool::new(false);
/// A check asked for now ("Check for updates"), and the worker's wake-up.
static ASKED: Mutex<bool> = Mutex::new(false);
static WAKE: Condvar = Condvar::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn set(status: Status) {
    let mut current = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    if *current != status {
        *current = status;
        CHANGED.store(true, Ordering::Release);
    }
}

/// Waits `delay` or until a check is asked for.
fn wait(delay: Duration) {
    let asked = ASKED.lock().unwrap_or_else(|e| e.into_inner());
    let (mut asked, _) = WAKE
        .wait_timeout_while(asked, delay, |asked| !*asked)
        .unwrap_or_else(|e| e.into_inner());
    *asked = false;
}

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
                wait(FIRST_CHECK);
                loop {
                    // A release already found stays offered while it checks.
                    if !matches!(status(), Status::Found(_)) {
                        set(Status::Checking);
                    }
                    let next = match check(commit) {
                        Ok(Some(release)) => {
                            set(Status::Found(release));
                            EVERY
                        }
                        Ok(None) => {
                            set(Status::Current);
                            EVERY
                        }
                        Err(error) => {
                            tracing::info!(%error, "Hexium update check failed");
                            set(Status::Failed);
                            RETRY
                        }
                    };
                    wait(next);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not start the Hexium update check");
        }
    });
}

/// Checks now instead of at the next interval ("Check for updates").
pub fn check_now() {
    if STARTED.get().is_none() {
        return;
    }
    *ASKED.lock().unwrap_or_else(|e| e.into_inner()) = true;
    WAKE.notify_all();
}

/// Where the check stands.
pub fn status() -> Status {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The newer release the last check found, if any.
pub fn available() -> Option<Release> {
    match status() {
        Status::Found(release) => Some(release),
        _ => None,
    }
}

/// Whether the status changed since the last call; the window redraws then.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
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
