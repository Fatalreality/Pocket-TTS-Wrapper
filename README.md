# Pocket TTS Wrapper

A Windows desktop wrapper for [Kyutai Pocket TTS](https://github.com/kyutai-labs/pocket-tts) — a
CPU-only neural text-to-speech engine — with a frosted-glass Tauri UI, 27 built-in voices,
zero-shot voice cloning and an in-app log console.

Made as a side project because a UI is nicer than a command line.

![Pocket TTS](docs/screenshot.png)

## Features

- **27 built-in voices**, filterable by gender, each with its own speaker embedding — no Hugging
  Face token needed, works fully offline.
- **Voice cloning** from a microphone recording, an audio file (wav/mp3/flac/ogg) or a
  `.safetensors` voice state, with export back to `.safetensors` for instant reuse.
- **CPU only**, with an int8 quantized model option for lower memory use.
- **Glass UI** with four themes and a three-way transparency switcher: transparent glass,
  high-visibility panels, and the Windows acrylic backdrop.
- **No console window** — engine logs stream into the collapsible in-app console instead.
- Playback with waveform, seek, duration/rate readout and WAV export.

## Requirements

- Windows 10 or 11, x64
- [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on
  Windows 11)
- To build: Rust stable with the MSVC toolchain, plus Node.js 18+ for the Tauri CLI

## Install

Download `Pocket TTS_0.1.0_x64-setup.exe` from
[Releases](https://github.com/Fatalreality/Pocket-TTS-Wrapper/releases), run it, and launch
**Pocket TTS** from the Start menu.

The installer is per-user (no admin prompt), installs into a folder of your choice, adds Start-menu
shortcuts and an uninstaller, and ships `README.txt` and `HF_TOKEN_GUIDE.txt` beside the app. A
portable `Pocket-TTS-*-portable.exe` is attached to each release as well.

## Build from source

```powershell
git clone https://github.com/Fatalreality/Pocket-TTS-Wrapper.git
cd Pocket-TTS-Wrapper

cargo build --release          # binary only
npx @tauri-apps/cli@2 build    # binary + NSIS installer in target/release/bundle/nsis
cargo test                     # engine unit tests
```

The frontend is plain static HTML/CSS/JS in `pocketGlass/ui` — there is no npm build step. Tauri
embeds it into the binary at compile time, so rebuild after editing any UI file. The model configs
from the upstream project are vendored in `configs/` and embedded the same way, so no separate
checkout of `pocket-tts` is needed to build.

## Voice cloning token

Cloning uses the gated `kyutai/pocket-tts` weights on Hugging Face:

1. Accept the terms at <https://huggingface.co/kyutai/pocket-tts>.
2. Create a **Read** token at <https://huggingface.co/settings/tokens>.
3. Paste it into **Voice Clone → Hugging Face Access** in the app and press **Save**.

The token is stored in `~/.cache/huggingface/token` and reused. Full walkthrough in
[`docs/HF_TOKEN_GUIDE.txt`](docs/HF_TOKEN_GUIDE.txt).

## Shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl` + `Enter` | Generate speech |
| `Space` | Play / pause |

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/` | Rust side: Tauri commands, model/voice state, audio I/O, text chunking |
| `src/main.rs` | Window setup, Windows glass/acrylic chrome, command registration |
| `pocketGlass/ui/` | Frontend (`index.html`, `app.js`, `styles.css`), embedded at compile time |
| `configs/` | Model YAML configs vendored from upstream Pocket TTS, embedded at compile time |
| `docs/` | User guides bundled into the installer, plus the README screenshot |
| `capabilities/`, `src-tauri/` | Tauri permissions and app icon |
| `dont-touch/` | Local-only leftovers: old duplicate UI, DWM probe crate, screenshots, attachments |
| `github-release/` | Local staging for release assets (installer, portable exe, guides) — not committed |

## Credits & license

TTS engine and model weights by [Kyutai](https://kyutai.org/) (Pocket TTS); inference through
[`xn-ptts`](https://github.com/gabrieldemarmiesse/xn-ptts). Voice embeddings come from Kyutai's
released voice packs and remain subject to their terms. This wrapper's own code is provided as-is
for personal use.
