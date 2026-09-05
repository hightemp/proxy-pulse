# Release automation and import reference verification

Date: 2026-09-05. Current source version: 0.1.0.

## Implemented

- VERSION-driven package, Cargo, Tauri and frontend versioning. Normal build/development commands synchronize metadata first; raw Cargo desktop builds reject stale version metadata.
- A tag-triggered GitHub Actions workflow for Linux Debian/AppImage, Windows NSIS and separate macOS arm64/x86_64 DMGs. Manual dispatch retries an existing tag.
- `make release`, `make release-dry-run` and `make release-notes`, including clean/pushed-commit checks, annotated tags, non-forced push and linked commit descriptions.
- Complete asset validation, SHA256SUMS, draft uploads and publication only after all matrix builds/uploads succeed.
- A detailed Supported formats dialog reachable from the importer, with state-preserving Back to import navigation.

## Local evidence

| Check | Result |
| --- | --- |
| Release/version tests | 15 passed, using temporary Git repositories and local bare remotes |
| Rust tests | 23 passed across the workspace |
| Protocol fixtures | 39 cases passed |
| Browser UI smoke | 3 existing tests passed |
| Static checks | Clippy, TypeScript, formatting and actionlint 1.7.12 passed |
| Linux packaging | Actual .deb and AppImage built successfully; AppImage is approximately 79 MiB |
| AppImage execution | Actual AppImage launched through extract-and-run mode in native WebDriver/Xvfb |
| AppImage workflow | Local HTTP/SOCKS checks, clipboard groups, JSON report, file save/reopen, search and cancellation passed |
| Version parity | VERSION = native Tauri app version = visible UI version, all 0.1.0 |
| Import help | Seven format groups rendered; input text, selected format, duplicate setting and parsed preview survived opening and closing help |

Release tests cover SemVer errors, metadata synchronization, external lockfile preservation, first/subsequent commit lists, dry run, rejected/duplicate/conflicting tags, safe push retry, tag/version mismatch, missing AppImage, draft upload failures, immutable published releases and prerelease classification.

The native chooser test now pastes paths with xclip. Simulated per-character typing could trigger GTK filename completion and alter the path; this was a test-driver issue, not a change to the application's file handling.

Generated evidence lives under ignored `artifacts/`: `release-ui-results.json`, `native-results.json`, `release-local-assets.json`, `release-format-help.png` and `release-import-return.png`. Local package checksums are written to `target/release/bundle/SHA256SUMS`, using paths relative to that directory.

## Scope limits

No tag was created or pushed in this project, and no public GitHub release was created. Real tag/push operations in tests were confined to temporary local repositories. The configured origin is used only to generate the repository's commit links during preview.

The remote workflow has not run yet. Windows/macOS builds, signing behavior and GUI acceptance therefore remain unverified. macOS is configured for ad-hoc signing; no Apple notarization or trusted Windows signing identity is configured.

The local Linux build uses Ubuntu 24.04 system libraries. The workflow's Ubuntu 22.04 AppImage baseline is configured but still needs its first remote run. A successful local AppImage test is not proof of compatibility with every Linux distribution.

For the release commands and required commit/push sequence, see [the release guide](../releases.md).
