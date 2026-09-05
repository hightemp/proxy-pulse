# Development startup with proxy environment variables

Date: 2026-09-05. Platform verified: Linux x86_64, WebKitGTK 2.52.6.

## Failure and cause

The development window requested `http://127.0.0.1:1420/` through the shell's HTTP proxy. That SSH proxy tried to open its own loopback address and returned a connection refusal. The reported error was the proxy response displayed in the WebView, rather than the application interface.

The inspected shell had HTTP_PROXY and HTTPS_PROXY configured, with neither NO_PROXY nor no_proxy. The previous native acceptance tests cleared proxy variables for WebDriver transport and therefore did not cover the development WebView under those conditions. A hostile-proxy baseline also blocked WebKit's local inspector connection before automation could attach.

## Change

The first operation in `main` adds `localhost`, `127.0.0.1`, `::1`, `[::1]`, `ipc.localhost`, `tauri.localhost` and `asset.localhost` to the application's proxy-exclusion list. Existing uppercase and lowercase exclusion lists are retained and supplied to both environment-variable casings. No system files or parent-shell settings are modified, and HTTP/HTTPS/ALL_PROXY values are left intact.

This happens before Tauri, WebKit, plugins or worker threads initialize. The checker's explicit `noproxy("")` setting continues to override ambient exclusions for actual proxy checks. The [libcurl no-proxy documentation](https://curl.se/libcurl/c/CURLOPT_NOPROXY.html) describes this explicit override.

## Validation

- Four Rust tests cover missing lists, preservation of both existing lists, empty/wildcard values and lossless handling of non-Unicode OS environment values.
- `make test-startup` loads the actual development URL in a native Tauri WebView, not an embedded release page or a browser mock.
- Cases: uppercase HTTP_PROXY/HTTPS_PROXY; lowercase http_proxy/https_proxy/all_proxy; conflicting pre-existing NO_PROXY/no_proxy lists.
- Every case must display the application, execute a real Rust IPC command and open Settings while sending zero UI requests to the proxy trap.
- The same application then checks that explicitly selected trap proxy against a loopback target. Exactly one request must reach the selected proxy, yielding the expected target HTTP error. A bypass that incorrectly made the check direct would fail this assertion.
- All three native cases passed: zero UI requests and exactly one explicitly selected checker request reached the trap in each case. All 23 workspace tests and Clippy/type checks passed.
- A final ordinary `make dev` launch inherited the user's actual HTTP_PROXY/HTTPS_PROXY settings on the desktop display. The application interface loaded successfully; a window capture is saved as `artifacts/proxy-startup-live.png`.

The test manages an isolated Xvfb/WebDriver instance and only stops processes it starts. Proxy endpoints and settings are synthetic. Detailed results and screenshots are written under ignored `artifacts/proxy-environment-*` paths.

Restart `make dev` or `pnpm desktop` to load the corrected executable. Windows/macOS startup behavior has not been independently verified in this regression run.

This change rebuilt and verified the development executable. Previously generated release packages were not rebuilt for this development-startup regression.
