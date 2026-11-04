# Hexium

Local voice dictation for Mac, Windows and Linux, built for how India speaks.
Hold a key, talk in Telugu, Hindi, Tamil or English (or all of them at once),
let go, and it pastes.

Hexium is a fork of [Hex](https://github.com/anomalyco/hex). It stays close
to Hex and pulls in its changes every week, adding:

- **Indian languages, mixed with English.** Indic-Transcribe-Flex is built in
  and picked for Telugu, Hindi, Tamil, Kannada, Malayalam, Marathi, Gujarati,
  Punjabi, Urdu and Bangla. It writes each language in its own script and the
  English you mix in as English.
- **Nothing phones home.** No telemetry, no update checks, no AI-assistant
  add-ons. The only network use is downloading a speech model the first time.
- **Installs beside Hex.** Its own name, icon, settings and data folder.

Everything still runs on your device.

## Download

Grab the latest build from [Releases](https://github.com/TheAnimatrix/hexium/releases):
`hexium-setup-…-windows-x64.exe` for Windows, the `.dmg` for Apple silicon
Macs, or the Linux executable, `.deb` or `.rpm`. Builds are unsigned, so
Windows SmartScreen and macOS Gatekeeper will ask once.

## Contributing

PRs are welcome, and we keep things moving fast: small, focused changes get
merged quickly. Open an issue if you'd rather just describe the problem.

## Credits

- [Hex](https://github.com/anomalyco/hex) by Anomaly Innovations, MIT.
- [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp), MIT.
- Built with Indic-Transcribe-Flex from Bodhan AI / AI4Bharat, under the
  [Indic Open Model License](https://github.com/Bodhan-AI/bodhan-model-info/blob/main/licenses/indic-open-model-license/v1/Indic_Open_Model_License.md).

Hexium's code is MIT licensed. More docs are in [`docs/`](docs/README.md);
how the fork is kept in sync is in [`hexium/`](hexium/README.md).
