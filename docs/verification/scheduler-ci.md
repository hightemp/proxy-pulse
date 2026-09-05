# Scheduler CI fixture corrections

Date: 2026-09-05. Source version remains 0.1.1; existing release tags are unchanged.

## Reported failures

- Ubuntu 22.04: the integration test required adjacent server handler timestamps to be at least 90 ms apart.
- macOS arm64: a handler rejected the bytes returned by a single socket read, then assertions and failed joins caused another panic during destructor cleanup and aborted the test process.

## Diagnosis

Server handler start times include connection delivery and thread scheduling delays. Those delays can bunch otherwise correctly paced client requests together, so handler timestamp gaps cannot enforce the client admission interval.

The test listener used nonblocking mode but did not normalize accepted sockets. BSD/macOS accept semantics can inherit socket properties, while Linux differs for O_NONBLOCK; portable code must set the desired mode explicitly. See [Apple's accept manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html) and the [Linux accept notes](https://man7.org/linux/man-pages/man2/accept.2.html).

The old fixture also converted every read error to zero bytes and assumed a single read contained the request prefix. TCP reads can be short, and Interrupted must be retried; see the [Rust Read contract](https://doc.rust-lang.org/std/io/trait.Read.html). The supplied macOS log did not record the exact first read result, so it cannot distinguish WouldBlock from a short read on that particular attempt. Both defects are now handled.

## Changes

- Factored the existing locked admission decision into a private method that accepts a clock function. Production passes Instant::now; tests provide deterministic times. Rate values and spacing are preserved, and cancellation/deadline are checked under the lock.
- Added exact interval-boundary tests, simultaneous-worker sharing, cancellation/deadline checks and protection against catch-up bursts after a pause.
- Separated network concurrency from rate verification. The rate integration test uses immediate responses and checks the minimum total duration for six requests at 10/s; it no longer measures server thread arrival gaps.
- Accepted fixture streams explicitly use blocking I/O with short bounded read timeouts. The fixture accumulates complete HTTP headers and handles short reads, temporary errors and cancellation disconnects.
- Fixture failures are recorded and asserted explicitly after joining. Drop only cleans up, so it cannot mask the original assertion with a second panic.
- Pending-response counters are released before writing the response, avoiding overcounting a completed client request while its server thread is still being cleaned up.

## Validation

- 33 workspace Rust tests passed: 4 startup, 4 pacing, 17 contract and 8 scheduler/fixture tests.
- 39 local protocol cases and 15 release automation tests passed.
- Clippy passed with warnings denied.
- Pacing and scheduler test binaries passed 20 consecutive repetitions pinned to one CPU with two competing CPU-bound processes. The fixture regressions cover one-byte fragments, transient read errors, initially nonblocking sockets, incomplete disconnects and cleanup during an existing panic.

This verification ran on the local Linux x86_64 environment. It is not a native macOS or Ubuntu 22.04 rerun. No workflow checks were disabled, and the one-second application cancellation criterion remains in place.

## Next release run

Commit the fix and prepare a fresh version/tag, for example 0.1.2, using [the release procedure](../releases.md). Rerunning the existing v0.1.1 tag checks out its original commit and cannot include these edits. Existing tags were not moved or republished during this fix.
