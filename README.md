# Proxy Pulse

A local desktop proxy checker built with Tauri, Rust and React. Import a mixed list, verify real requests through each proxy, and copy or save the results.

The first Linux implementation is available.

## Run the application

Requirements: Rust 1.90.0, Node 22.16.0+, pnpm 10.32.1 and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/). On Ubuntu 24.04, install development packages for WebKitGTK 4.1, GTK 3, libcurl/OpenSSL, libxdo and librsvg.

```sh
pnpm install --frozen-lockfile
make doctor
make dev
```

`make dev` opens the native desktop app. `make preview` runs only the frontend in a browser; it clearly disables desktop operations and does not fabricate check results.

The desktop process automatically adds loopback and Tauri's local hosts to both `NO_PROXY` and `no_proxy` before WebKit starts. Existing exclusions and HTTP/HTTPS proxy settings are preserved. This keeps the development UI at `127.0.0.1:1420` local when your shell has `HTTP_PROXY` configured; no shell or system configuration changes are needed. The checker still uses each explicitly selected proxy.

```sh
make build-debug  # Standalone debug executable: target/debug/proxy-pulse
make build        # Release executable: target/release/proxy-pulse
make package      # Linux Debian installer: target/release/bundle/deb/
make help         # All commands
```

Without Make, use `pnpm desktop`, `pnpm tauri build --no-bundle`, `pnpm tauri build --debug --no-bundle` and `cargo test --workspace --locked`. Native Windows/macOS packaging must be checked on those systems before distributing a supported package.

## Import

Use **Add proxies**, **Import file**, clipboard paste, or drop a UTF-8 TXT/CSV/TSV file onto the app. The preview shows valid records, errors and duplicates. Edit rejected records or keep them as Invalid rows for later correction. Appending is the default; replacing the list and keeping duplicates are explicit options.

```text
192.0.2.10:8080
proxy.example:1080:demo-user:demo-pass socks5
demo-user:demo-pass@proxy.example:1080
https://demo-user:demo-pass@proxy.example:8443
socks5h://demo%40user:p%3Ass@[2001:db8::10]:1080
socks4a://userid@proxy.example:1080
```

For CSV/TSV, use columns such as `host,port,username,password,protocol`, supported header aliases, or an explicit column mapping. Select the reverse-order template for `username:password:host:port`; it is never guessed. Select Text explicitly if a delimiter inside a compact record could be confused with CSV. Use an encoded URI or mapped fields for special characters in credentials.

Limits: 100,000 records, 20 MiB per input and 8 KiB per record. IPv6 endpoints require brackets and ports are mandatory. See the in-app **Formats & help** dialog and [PRD.md](PRD.md) for the full input contract.

## Check

Supported routes: HTTP, HTTPS (TLS to the proxy), SOCKS4, SOCKS4a, SOCKS5 with local DNS (`socks5`) and SOCKS5 with remote DNS (`socks5h`). Without a protocol, Auto tries HTTPS, SOCKS5 remote DNS, HTTP, SOCKS4a and SOCKS4. Candidates incompatible with supplied credentials are skipped. Explicit protocols are respected; **Detect again** changes a row to Auto.

The default check requests an IP echo endpoint over HTTPS. Settings also support custom HTTP/HTTPS URLs, response validation, a fallback URL, concurrency, pacing, deadlines and limited retries. Both certificate layers are verified. System and environment proxy settings do not select an alternative route for checks.

| Status | Meaning |
| --- | --- |
| Working | A real request passed the selected profile |
| Failed | A concrete connection, proxy, certificate or authentication error |
| Inconclusive | The endpoint failed, the protocol was not confirmed, or the client could not evaluate the proxy |
| Invalid | The record needs correction before a check |
| Cancelled | The check was stopped before completion |

Open a row for protocol attempts, the sanitized check URL and error details. Latency measures the successful attempt, including setup and response validation. It is not ICMP ping or bandwidth. Authentication validity is reported separately; a successful request does not automatically prove that a supplied password was required.

## Export and session data

**Copy working / Save working** use proxy URLs. **Copy failed / Save failed** preserve original records. **More** offers Checked, Inconclusive, Selected, Filtered and All scopes, with URLs, original lines, compact text, CSV reports and versioned JSON reports.

Checked includes Working, Failed and Inconclusive. It excludes Invalid and Cancelled. Working and Failed actions use the complete list, independently of the table filter.

Reusable lists include credentials by default; reports omit them by default. Unknown Auto protocols and incompatible source schemas require an appropriate format instead of silently dropping rows. CSV reports escape spreadsheet formula prefixes; URLs and JSON preserve exact credential data.

Proxy lists, passwords and custom profiles live in memory. Only theme, concurrency and request pacing are saved automatically. Save a file before quitting to retain data. Reports are not a session-restore format yet. There is no telemetry, cloud account or system proxy switcher.

## Verify

```sh
make quality           # Format, lint, types, core contracts, local protocols, browser UI
make test-integration  # Real loopback proxy matrix; no public proxies
make test-ui           # Browser layout/help tests (not native protocol evidence)
```

Python 3 and OpenSSL are required for protocol fixtures. Browser checks use local Chrome when available, or an installed Playwright Chromium (`pnpm exec playwright install chromium`). `CHROME_PATH` can select a test browser. The UI test wrapper removes proxy environment variables only for its child process, so loopback readiness checks are not sent through an external proxy.

For native Linux GUI acceptance, install a matching WebKitWebDriver and tauri-driver into a test tools directory, then run:

```sh
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy \
  xvfb-run -a tauri-driver --native-driver /path/to/WebKitWebDriver --port 4457 --native-port 4458
# In another terminal, after make build-debug:
make test-native
```

The native script exercises actual Rust IPC, local proxies, the rendered WebView, clipboard groups, reports, search and cancellation. Results are written under ignored `artifacts/`. [Initial Linux evidence](docs/verification/initial-linux.md) distinguishes completed checks from remaining work.

With xdotool available (on PATH or under `artifacts/xdotool/extracted/usr/bin/`), the Linux native test also drives the save/open choosers. It discovers the matching driver's isolated display and reports this optional coverage as `native_file_dialogs_verified`.

`make test-startup` builds the development executable and tests its real WebView with uppercase/lowercase proxy variables and pre-existing exclusions. It manages its own Vite/WebDriver processes, uses an isolated proxy trap and verifies both local UI loading and explicit checker routing. It requires Xvfb, tauri-driver and WebKitWebDriver, found on PATH or in the test tool locations under `artifacts/`. See [the startup regression report](docs/verification/proxy-environment.md).

## Structure

```text
crates/core/         Parser, model, checker, scheduler/session and export
src-tauri/           Desktop commands, native dialogs, clipboard and capabilities
src/                 English React interface and virtualized result table
scripts/             Offline fixtures, native smoke and development utilities
tests-ui/            Browser UI smoke checks
docs/adr/            Platform, network and result-semantics decisions
docs/verification/   Recorded acceptance evidence and limits
```

Remaining work includes full 100,000-row GUI/RSS measurements, richer profile snapshots and independent fallback validators, remaining context-menu workflows, further native dialog edge cases, and Windows/macOS packaging and acceptance. Consult TASKS.md for precise completion boundaries.
