# Release

## Decisions

- Minimum macOS is 14.0. Per-bench WebKit data stores (`data_store_identifier`) need macOS 14, and the session isolation in ARCHITECTURE.md depends on them. Pilot's own macOS route uses current Homebrew.
- Target is Apple Silicon (`aarch64-apple-darwin`). Intel (`x86_64-apple-darwin`) is not configured: Homebrew's `rust` has no second target and no Intel Mac was available. Adding it needs `rustup target add x86_64-apple-darwin` and `npm run tauri:build -- --target x86_64-apple-darwin`, then a native test run on Intel hardware.
- Pilot Desktop and Pilot update separately. Neither updates the other, and no Frappe app is upgraded automatically.

## Local build (no signing credentials)

```bash
cd desktop
npm ci
npm run tauri:build
```

Outputs, under `src-tauri/target/release/bundle/`: `macos/Pilot Desktop.app` and `dmg/Pilot Desktop_<version>_aarch64.dmg`. Without signing credentials the build is ad-hoc signed. macOS Gatekeeper warns on first open: right-click the app and choose Open.

## Product name and bundle identifier

Both are configuration, not code. The defaults live in `src-tauri/tauri.conf.json` (`productName`, `identifier`). Override at build time without editing files:

```bash
npm run tauri:build -- --config '{"productName":"Acme Pilot","identifier":"com.acme.pilot"}'
```

`src-tauri/tauri.identity.example.json` shows the shape. Pick the final identifier before the first signed release, because macOS keys application support data and notarization history to it.

## Developer ID signing and notarization

Never commit credentials. Provide them as environment variables, or as CI secrets for `.github/workflows/desktop-release.yml`, which is manual (`workflow_dispatch`) and never publishes.

| Variable | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64 of the Developer ID Application `.p12` (CI only) |
| `APPLE_CERTIFICATE_PASSWORD` | Password for that `.p12` (CI only) |
| `APPLE_SIGNING_IDENTITY` | For example `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Notarization with an app-specific password |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH` | Notarization with an App Store Connect API key, instead of the three above |

With these set, `npm run tauri:build` signs, submits for notarization and staples. The workflow then runs `codesign --verify --deep --strict`, `spctl --assess` and `xcrun stapler validate`. Publishing the artifacts is a separate, deliberate step.

## Data and updates

- Preferences, ownership records and logs are stored under `~/Library/Application Support/<identifier>` and `~/Library/Logs/<identifier>`, never inside the `.app`.
- Replacing or deleting the app removes none of: Pilot, benches, databases, repositories.
- Preferences carry `schemaVersion`. A newer app reads older files tolerantly, and a corrupt file is kept as `preferences.json.corrupt`.
- Quitting stops the benches this app started and leaves attached ones running.

## Included dependencies

Frappe UI 1.1.0 (MIT), Vue 3.5 (MIT), Tailwind CSS 3.4 (MIT), Tauri 2 and its plugins (MIT or Apache-2.0), the Rust crates in `src-tauri/Cargo.lock`, and the npm packages in `package-lock.json`. Inter fonts ship with Frappe UI (SIL OFL 1.1). The project is distributed under the Pilot repository's AGPL-3.0 license. The app icon is a placeholder to be replaced by official branding.

## Guided installation source

Install Pilot runs inside the app, with no Terminal. After one confirmation it works through only the missing steps: Apple developer tools (`xcode-select --install`, macOS shows its own window), Homebrew (the official installer, with `SUDO_ASKPASS` pointing at a native macOS password dialog so the password never passes through the app), Node.js 24 (`brew install node@24`, put first on the `PATH` Pilot children see, so an older nvm Node does not matter), Pilot (`https://raw.githubusercontent.com/frappe/pilot/develop/install.sh`, saved to the support folder with its SHA-256 and recorded in `installer/installation.json`), and Python (`uv python install 3.14`). The URL tracks a branch, so the script can change between runs. Network failures are retried up to three times. If `~/pilot` is a git checkout the installer runs with `--dev` and updates it in place, so a release download never overwrites a developer's files. If no release asset exists it falls back to `--dev`. If that checkout cannot update (local changes, or an upstream branch that no longer exists) setup stops with a plain explanation and offers Move it aside and continue, which renames `~/pilot` to `~/pilot (previous)` after confirmation. Nothing is deleted, and a folder that contains benches is never moved.

## Checked-in build

`artifacts/macos-arm64/` holds the unsigned, ad-hoc signed Apple Silicon build of the current source: the DMG, the app as a zip, and `SHA256SUMS`. Gatekeeper warns on first open. Rebuild with `CI=true npm run tauri:build` and refresh these files after any source change. Signed and notarized releases belong in a GitHub release, not in git.

