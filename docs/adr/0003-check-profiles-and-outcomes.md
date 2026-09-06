# ADR 0003: Check profiles and result semantics

Date: 2026-09-05. Status: implemented baseline, with remaining PRD work tracked in TASKS.md.

The default profile sends GET to `https://api64.ipify.org?format=json` and requires HTTP 200 plus a JSON `ip` field containing a valid IP address. The response format follows [ipify's published API](https://www.ipify.org/). A custom profile can select HTTP or HTTPS, an expected status, optional body text and the IP validator. The current fallback URL uses the same validation rules as the primary URL; independent fallback validators remain planned.

Working means that the current request satisfied the profile. Failed identifies a concrete connection, authentication, proxy certificate or tunnel rejection. Inconclusive identifies insufficient protocol evidence, an unsupported client capability, a target failure, a target certificate problem or an exhausted detection budget. Invalid and Cancelled are separate.

The result stores the settings snapshot internally. The UI receives a sanitized endpoint URL with its query removed. As of the 2026-09-06 storage requirement, profile URLs, body match strings, raw records and credentials are automatically persisted in the private local workspace and portable backups; the row-view IPC still excludes secrets. See [storage and backups](../storage.md). Export reports also sanitize the endpoint URL.

For automatic detection, the first complete success wins. A cancelled or incomplete run remains cancelled/inconclusive. Confirmed authentication/tunnel errors and invalid proxy certificates take precedence over generic timeouts from unrelated candidates. A target failure on an established path takes precedence over unconfirmed connection failures from other candidates. All attempts remain available for diagnosis.

Headers are limited to 32 KiB; decoded response bodies to 64 KiB. Redirects are disabled. Fallback requests use the same proxy and the remaining total budget. Both TLS layers validate trust and hostnames.

Each run keeps a sliding window of up to 20 HTTP responses per endpoint. At least five 429/5xx responses comprising at least 80% of that window stop new requests to that endpoint in the run. A configured fallback is still available. Already completed results are not rewritten.

The public service is not used for load tests. Unit, protocol and GUI tests use local endpoints. The current project does not provide independent proof of the public service's availability through every user's network.
