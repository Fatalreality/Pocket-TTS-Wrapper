# Releasing Pocket TTS

Releases are built and published by `.github/workflows/release.yml` when a `v*` tag is pushed.
The workflow builds the NSIS installer and the portable executable, signs the update, stages the
assets, and publishes the GitHub release.

## One-time setup: the updater signing key

The in-app updater only installs a build whose signature it can verify, so every release must be
signed with the same private key. A keypair was generated into `dont-touch/updater.key` (git
ignored) and its public half already sits in `tauri.conf.json` as `plugins.updater.pubkey`.

Add two repository secrets under **Settings → Secrets and variables → Actions**:

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | the contents of `dont-touch/updater.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the key's password (currently empty) |

Without the first secret the build produces no `.sig` file and `stage` stops with an error,
because an unsigned release would break updates for everyone on an older build.

Losing or replacing this key means existing installs can no longer verify updates: they have to
be reinstalled by hand once from the new release. Keep a copy somewhere safe.

## Cutting a release

```bash
node scripts/release.mjs set-version 0.2.2   # tauri.conf.json, Cargo.toml, Cargo.lock, README
# rename "## [Unreleased]" in CHANGELOG.md to "## [0.2.2] - YYYY-MM-DD" and add a fresh Unreleased
node scripts/release.mjs notes               # check the notes that will be published
git add -A && git commit -m "Release 0.2.2"
git tag -a v0.2.2 -m "Pocket TTS 0.2.2"
git push origin main --follow-tags            # the tag starts the release workflow
```

The workflow needs no arguments: it reads the version from `tauri.conf.json`, takes the notes
from the matching `CHANGELOG.md` section, and uploads these assets:

| Asset | Source |
| --- | --- |
| `Pocket-TTS-<v>-x64-setup.exe` | the NSIS bundle, renamed without spaces |
| `Pocket-TTS-<v>-portable.exe` | `target/release/pocket-tts-app.exe` |
| `README.txt`, `HF_TOKEN_GUIDE.txt`, `app-icon.webp` | `docs/` |
| `SHA256SUMS.txt` | checksums of everything above |
| `latest.json` | the updater manifest, pointing at the signed installer |

## Building a release locally

```bash
# The private key is only needed because the build signs the updater payload.
export TAURI_SIGNING_PRIVATE_KEY="$(cat dont-touch/updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npx @tauri-apps/cli@2 build

node scripts/release.mjs stage github-release
GITHUB_TOKEN=... node scripts/release.mjs publish github-release
```

`publish` creates the release if the tag has one already (or updates it), replaces any asset with
the same name, and sets the notes from the changelog. It uses `GITHUB_TOKEN` or, locally, the
credentials `git credential fill` returns for github.com.

## Verifying a release

- `gh release view v<version>` (or the API) shows every asset as `uploaded`.
- Every asset downloads: a ranged `GET` should answer `206`.
- `SHA256SUMS.txt` matches the downloaded installer and portable binary.
- `latest.json` has `version` equal to the release and `platforms.windows-x86_64.url` pointing at
  the installer in that same release.
- Run the installer once, launch the app, and use the update button: a newer build must be
  reported and install itself.
