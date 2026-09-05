# ADR 0001: Desktop platform and toolchain

Date: 2026-09-05. Status: accepted for the Linux implementation; other native platforms await verification.

Use Tauri 2 with a React/TypeScript/Vite interface and a separate Rust core crate. The core has no dependency on Tauri and is exercised by offline fixtures. The application runs as the current user and changes no system proxy configuration.

| Component | Selected version |
| --- | --- |
| Rust | 1.90.0, pinned in rust-toolchain.toml |
| Node | 22.16.0 or a compatible newer Node 22 runtime |
| pnpm | 10.32.1 |
| React | 19.2.8 |
| TypeScript | 5.9.3 |
| Vite | 6.4.3 |
| Tauri Rust / CLI / JS API | 2.11.5 / 2.11.4 / 2.11.1 |

Cargo.lock and pnpm-lock.yaml define the resolved dependencies. Cargo's incompatible-Rust-version policy uses compatible packages rather than requiring an implicit toolchain upgrade.

| Target | Baseline | Build | GUI | Packaging |
| --- | --- | --- | --- | --- |
| Linux x86_64 | Ubuntu 24.04, GTK 3, WebKitGTK 4.1 | Local debug/release workflow | Native WebDriver smoke available | Debian package first |
| Windows x86_64 | Windows 10/11, MSVC, WebView2 | Not executed | Not executed | NSIS planned; signing unresolved |
| macOS arm64 | macOS 12+ | Not executed | Not executed | DMG planned; signing/notarization unresolved |
| macOS x86_64 | macOS 12+ | Not executed | Not executed | Separate native build planned |

The selected OS baselines are product targets, not claims of completed support. Tauri's underlying prerequisites are documented in the [official installation guide](https://v2.tauri.app/start/prerequisites/).

Native file selection and clipboard operations are mediated by Rust commands. Only the local application window receives Tauri capabilities. The frontend loads no remote scripts, fonts, pages or analytics.

Development commands are discoverable with `make help`. The equivalent pnpm/Cargo commands are provided in the README for environments without Make. Test-only WebDriver binaries are installed under ignored `artifacts/`, not into the user's global toolchain.
