# ADR 0002: libcurl-backed proxy checking

Date: 2026-09-05. Status: selected and exercised on Linux; platform-specific packaging remains open.

Use the Rust `curl` crate (0.4.50) with libcurl. Do not launch a curl subprocess for application checks. The current Linux executable links the distribution's libcurl 8.5.0 and OpenSSL 3.0 libraries; the curl command installed in the user's shell is a different binary and is not runtime evidence for this application.

The [libcurl proxy configuration API](https://curl.se/libcurl/c/CURLOPT_PROXY.html) explicitly distinguishes HTTP, HTTPS, SOCKS4, SOCKS4a, SOCKS5 and SOCKS5 with remote DNS. [Reqwest's proxy API](https://docs.rs/reqwest/latest/reqwest/struct.Proxy.html) was considered as an alternative; it was not prototyped against the complete acceptance matrix. libcurl was selected after the local protocol fixture succeeded, without claiming that the alternative cannot support these features.

Each worker owns a libcurl multi handle and a single active transfer. The worker drives the event loop with short waits, checks cancellation and an outer deadline, and drops the transfer before leaving. Automatic candidates run sequentially. A shared request limiter applies across workers, candidates, retries and fallback endpoints.

The application explicitly supplies a proxy URI and clears the no-proxy exclusion list. The fixture process deliberately receives upper- and lowercase HTTP_PROXY, HTTPS_PROXY, ALL_PROXY and NO_PROXY settings pointing to a trap server. No connection to the trap is allowed. A destination name resolvable only inside the fixture distinguishes remote from local DNS.

SOCKS5 authentication is explicitly restricted to username/password (alongside no authentication). The fixture inspects the offered methods and rejects GSSAPI. A small documented FFI adapter sets CURLOPT_SOCKS5_AUTH because the safe wrapper does not expose that option.

| Capability | Linux evidence |
| --- | --- |
| HTTP forward and CONNECT | Successful requests to separate local HTTP/HTTPS targets |
| TLS to HTTPS proxy, TLS inside tunnel | Both TLS layers accepted with an explicit fixture CA; independent trust failures rejected |
| SOCKS4 / SOCKS4a / USERID | Local requests completed |
| SOCKS5 local / remote DNS | Requests completed; remote-only name succeeds only in remote mode |
| Basic / SOCKS5 username-password | Correct credentials succeed; incorrect credentials fail |
| Explicit vs automatic protocol | Both exercised; explicit mode does not fall back |
| Target 403/429/503, malformed/oversized response | Inconclusive results; fallback remains proxied |
| Cancellation | Real GUI smoke stops an active run and preserves completed rows |

See `scripts/network_fixtures.py` and [the acceptance report](../verification/initial-linux.md). The fixture's custom CA is supplied only by the standalone core test harness, never by a disable-verification UI option.

Known limitations to resolve before declaring every PRD criterion complete:

- libcurl's connect timeout includes protocol negotiation; it is not yet an independently enforced TCP-only timeout followed by a separate handshake budget.
- Some SOCKS diagnostics are mapped from bounded, discarded libcurl diagnostic text. Exact low-level method/address rejection coverage must be expanded across library versions.
- Successful requests with supplied credentials can report `Not tested` for authentication validity when the library does not expose whether those credentials were required. Success is never promoted to confirmed password validity by assumption.
- Native Windows/macOS TLS, DNS, cancellation and dependency packaging have not been tested.

Only boolean diagnostic flags are retained from verbose callbacks. Raw debug text, headers and credentials are never forwarded to the UI or logs.
