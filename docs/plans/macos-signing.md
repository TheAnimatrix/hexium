# macOS signing under Anomaly

## What Hex does

```ts
scripts/build-app.sh       // cargo build → Hex.app; signs Moonshine and ONNX dylibs,
                           // HexMicrophone.driver, then the app with --options runtime
                           // and .github/release/hex.entitlements (audio-input,
                           // apple-events; not sandboxed)
                           // HEX_TEST_BUILD=1: ad-hoc "Hex Test.app" (anomaly.hex.test)
scripts/macos-signing.sh   // identity must be "Developer ID Application: … (TEAM)"
scripts/release-app.sh     // clean pushed tree → notarized, stapled app and DMG
                           // (scripts/notarize-app.sh) → dist/hex-<v>-macos-arm64.dmg
                           // and .zip (root Hex.app, the updater's archive)
scripts/validate-app.sh    // bundle id anomaly.hex, team, Developer ID, entitlements,
                           // no Sparkle left
release.yml        // build-macos: HEX_RELEASE_UNSIGNED=1 build-app.sh, no secrets
                           // sign-macos (release env, .github/release only):
                           // macos-credentials.sh imports APPLE_SIGNING_CERTIFICATE and
                           // APPLE_NOTARY_KEY; sign-macos.sh signs inside-out,
                           // notarizes, staples, makes the ZIP and DMG
                           // verify-macos: Gatekeeper, staples, validate-app.sh, the
                           // updater's requirement (app/update-requirement.txt), launch
                           // A check run is the signed candidate; publishes nothing
test-builds.yml            // per-push ad-hoc Hex Test DMG (unsigned)
```

Next: once the `APPLE_*` secrets exist, run a staging Release and check the
signed build against the updater's requirement (verify-macos does); then
optionally sign `Hex Test.app` with the same Developer ID so testers grant
permissions once (needs `build-app.sh` to accept a team with
`HEX_TEST_BUILD=1`).

## Secrets for `anomalyco/hex`

```ts
APPLE_CERTIFICATE           // base64 .p12 of the Anomaly "Developer ID Application" cert + key
APPLE_CERTIFICATE_PASSWORD  // the .p12 password
APPLE_API_KEY               // that key's ID
APPLE_API_ISSUER            // the App Store Connect issuer ID
```

`APPLE_ID`, `APPLE_PASSWORD`, and `KEYCHAIN_PASSWORD` are not needed.

## New identity: `anomaly.hex`

The first Anomaly-signed release also changes the bundle ID, in the same
release so users re-grant permissions once:

```ts
app            // com.kitlangton.hex2               -> anomaly.hex
SDK helper     // com.kitlangton.hex.service        -> anomaly.hex.service
mic driver     // com.kitlangton.hex.microphone.driver -> anomaly.hex.microphone.driver
mic UIDs       // com.kitlangton.hex.microphone[.sink] -> anomaly.hex.microphone[.sink]
test build     // com.kitlangton.hex2.test          -> anomaly.hex.test
Windows event  // Local\com.kitlangton.hex.app      -> Local\anomaly.hex.app
```

`anomaly.hex` is a new app to macOS. There is deliberately no migration code:
the old preferences domain (Sparkle state, a local `DisableUpdates`) is not
imported, managed `DisableUpdates` profiles must name `anomaly.hex`, and the
permissions and login item start fresh. The `hex` Application Support folder
does not depend on the bundle ID, so settings, history, models, and meetings
are shared, and its instance lock keeps an old and a new copy from running at
once (as does `windows-listener.lock` on Windows).

- **Launch at login.** `SMAppService.mainAppService` registers per bundle ID:
  the new app starts unregistered and Settings shows it off. An old
  `com.kitlangton.hex2` registration can linger in Login Items (Background
  Task Management) until macOS prunes it or the user removes it; when its app
  is gone or replaced it launches nothing. Unverified on a real Mac.
- **Hex Microphone driver (experimental).** It installs at the fixed path
  `/Library/Audio/Plug-Ins/HAL/HexMicrophone.driver`, so there is never a
  second copy. An old `com.kitlangton.hex.microphone.driver` there fails the
  bundle-ID check and reads as needing an update; the new app does not use its
  devices (different UIDs), and Update or Uninstall replaces or removes it. If
  the user never opens the Hex microphone card, the old driver stays installed
  and its "Hex Microphone" device stays in the system's device list.

## Existing installs

Macs on 2.3.1 or earlier run Sparkle with `com.kitlangton.hex2`; their
`SUFeedURL` redirects to `https://anoma.ly/hex/files/appcast.xml`. They never
auto-update into `anomaly.hex`: the site serves
`packaging/macos/legacy-appcast.xml` there, one informational item (no
enclosure) that links to https://anoma.ly/hex. Sparkle shows its notes and a
download button and installs nothing. It needs no Sparkle EdDSA key: the item has no
archive to sign and 2.3.1 does not set `SURequireSignedFeed`. Set the item's
version to the first `anomaly.hex` release before publishing.

## Implications of changing the team

- **Users must re-grant permissions.** TCC records each grant against the app's
  designated requirement, which names the signing team. After the first
  Anomaly-signed update, Microphone, Accessibility,
  Automation, Screen and System Audio Recording (Meetings) stop matching; old
  entries can remain listed but inert, so users may need to remove and re-add
  Hex. Onboarding's permission health gate must show this; release notes must
  say so.
- **Launch at login.** `SMAppService.mainAppService` registrations are tracked
  by Background Task Management with the team; expect the item to need
  re-enabling or approval after the switch. Hex polls macOS for the status
  (no stored flag), so Settings shows the truth; verify on a real Mac.
- **HexMicrophone.driver.** The bundled copy is re-signed by Anomaly; an
  installed copy in `/Library/Audio/Plug-Ins/HAL` keeps Kit's signature until
  the in-app Update reinstalls it.
- **Keychain items and app groups.** Hex stores none, so nothing else breaks.
