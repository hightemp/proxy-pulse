# Local workspace and backup verification

Date: 2026-09-06. Source version remains 0.1.3.

The application now saves proxies, credentials, original import metadata, latest results and result profiles, full check settings and appearance in its user data directory. See [the storage contract and recovery procedure](../storage.md).

## Automated checks

- 49 workspace Rust tests passed, including 16 new storage tests covering full and selective archives, special/empty credentials, IPv6, Auto, original CSV mappings, duplicates and invalid rows, result profile preservation, cancelled unfinished checks, field/version/report rejection, imports during a run, row limits, migration, restart and clear, previous-generation recovery, protection of unrecoverable files, instance locking, concurrent flushes, write failures and Unix 0700/0600 permissions.
- Workspace Clippy passed with warnings denied; TypeScript and production frontend build passed.
- Four browser UI tests passed, including keyboard access and the backup dialog at the minimum 1000×650 window size.
- A standalone Tauri debug executable was built and exercised through WebKitWebDriver, with real Rust IPC and native open/save dialogs.

## Native Linux acceptance

`scripts/storage_smoke.py` runs its own WebDriver/Xvfb instance and launches the app with temporary XDG data/config directories. It does not read or mutate the user's workspace. Test proxy credentials and check URLs are synthetic, with loopback endpoints only.

Verified behavior:

1. Import mixed proxies, make a real successful proxy request, and let the background worker save without an explicit flush command. Close through the native window event and restart the executable. The list, password, IPv6 row, invalid row, completed result, full settings and dark appearance return; ordinary row snapshots exclude the password and URL token.
2. Use the visible backup dialog and native save chooser to export full, proxies-only and settings-only JSON files. Verify the resulting file sections. Inspect the real dialog at 1000×650; content scrolls within the dialog and the footer remains in the window.
3. Change appearance and URL, then restore settings alone while the proxy list is empty. Restore proxies independently, merge the full backup without duplicates, then explicitly replace the list. All settings, records and results return. Invalid JSON is rejected with a visible error and preserves the list.
4. Prevent file replacement in the isolated data directory and edit a row. **Not saved** appears and the previous main file stays intact. Remove the obstruction; background saving recovers, and the edited row survives another restart.
5. Close during a controlled slow request using **Stop, save and quit**, then restart. The incomplete check is Cancelled and no network run resumes automatically.
6. Select two of three records, including an invalid row. **Remove selected (N)** is visible at 1000×650 and disabled during a check. Filter the selected rows out of view, cancel the confirmation once, then confirm removal. Only those two records disappear; selection clears and the remaining record survives restart. Screenshot: `artifacts/native-remove-selected.png`.

Artifacts: `artifacts/storage-results.json`, `artifacts/storage-smoke.log` and `artifacts/native-storage-backup.png`. The screenshot was visually inspected. The script is available through `make test-storage`; it requires Linux, Xvfb, WebKitWebDriver, tauri-driver, xclip and xdotool.

Existing native and proxy-environment test scripts also use isolated XDG data folders now that the application persists state.

These checks ran on Linux x86_64. They do not establish native Windows/macOS acceptance, encrypted storage, all-history retention or an installer/release build. VERSION, tags and GitHub releases were not changed.
