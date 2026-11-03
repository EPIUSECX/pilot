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

## Publishing builds

Builds are not kept in git. Unsigned local builds stay in `src-tauri/target/release/bundle` (and `artifacts/`, which git ignores). Signed releases come from the `Desktop release (signed)` workflow: run it manually, tick `publish`, and it attaches the DMG, the zipped app and `SHA256SUMS` to a draft GitHub release named `desktop-v<version>`. Review the draft, then publish it. Bump the version in `package.json` and `src-tauri/tauri.conf.json` first, because an existing tag makes the step fail rather than overwrite it. File names contain no spaces so the checksums verify after download. The first build with a sealed signature is `desktop-v0.2.0`.

Unsigned builds carry a sealed ad-hoc signature, set by `bundle.macOS.signingIdentity = "-"` in `tauri.conf.json`. Without it only the linker's partial signature exists, `codesign --verify` fails, and macOS reports a downloaded app as damaged. A Developer ID identity from the signed workflow overrides it. Ad-hoc builds are still not notarized: open them with System Settings > Privacy & Security > Open Anyway, or `xattr -dr com.apple.quarantine`. Both CI builds run `codesign --verify --deep --strict` on the app.
