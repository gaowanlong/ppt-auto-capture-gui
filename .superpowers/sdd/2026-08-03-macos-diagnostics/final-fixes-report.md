# macOS diagnostics final fixes

## Scope

- Rapid-transition pending-frame and fallback-reference save failures now emit `WorkerEvent::Error` messages with complete `anyhow` chains (`{error:#}`), allowing the existing application event flow to retain them in diagnostic history and copied reports.
- Worker event-channel disconnection is now handled once: both channels are cleared, the worker is stopped, the dashboard enters `Error`, the session becomes inactive, and the error is retained in history.
- Persistent diagnostic log files now include milliseconds plus a process-local monotonic suffix, preventing same-second or same-millisecond reuse.
- Retention attempts every expired, matching log file even if an earlier removal fails, then returns one error containing all failed paths. It never expands the matching/deletion scope.

## Test-first evidence

The new unit tests were added before the production helpers existed and were run in the failing state. They verify worker error events and error-chain rendering for both rapid-transition save paths, one-time disconnected-worker cleanup and history behavior, distinct successive log paths, and continued deletion attempts after an injected removal failure.

## Verification

- `cargo test app::tests --all-features`: 5 passed.
- `cargo test diagnostics::tests --all-features`: 9 passed.
- `cargo test capture::capture_worker::tests --all-features`: 4 passed.
- `cargo test --all-targets --all-features`: 126 passed.
- All shell and Ruby contract tests under `tests/` passed, including the macOS DMG end-to-end packaging test.
- `scripts/check-rustfmt-changed.sh` passed for the changed Rust files; `rustfmt --check` also passed for the three modified source files.
- Structured `cargo clippy --all-targets --all-features --message-format=json` reported 42 warnings, all with code `dead_code`.
- `git diff --check` passed.

## Baseline note

The checked-in Clippy baseline is 47 while this host observes 42 retained `dead_code` warnings. The baseline checker therefore requests a reduction, but it was intentionally left unchanged because this final-fixes task explicitly forbids baseline changes.
