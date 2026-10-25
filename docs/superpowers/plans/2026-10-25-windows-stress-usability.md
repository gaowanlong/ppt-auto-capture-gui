# Windows stability and usability implementation plan

Goal: pressure-test the existing capture application and make everyday capture and recovery operations dependable on Windows.

Scope: retain Rust/egui, GDI capture, session journals, existing PPTX generation and lock/unlock recovery. No further WeLink/locked-window exploration. Prior authorization permits implementation, review, merge and push without additional confirmations.

1. Bound worker-to-UI notifications, including status/error/preview storms. Coalesce snapshots while retaining latest slide count, stop state, output path and a bounded recent error history. Sending must never wait for a slow UI. Preserve channel disconnection and shutdown behavior. Test a 100,000-event storm and cloned endpoint lifetimes.
2. Repair test screenshot workflow and preview geometry, duplicate-save state notifications, worker configuration normalization and failed-start states. Test screenshot must work before recording starts and use the same source/backend selection. Validate preview buffers before rendering. Do not clear an active session's counters or pending saves. Cover wide/tall/tiny previews, malformed data, backend failure and configuration extremes.
3. Add Windows-native stress coverage of real screen/window capture, success and error cleanup, repeated initialization, worker start/stop and storage/recovery faults. Provide a PowerShell entry point supporting a local 24-hour run with JSON metrics, no stored desktop images, and a bounded-duration manual GitHub workflow with uploaded reports. Fast regression gates remain on every push. Separate simulated frame counts from elapsed native runtime in reports and documentation.

Verification: demonstrate failures first for fixes; run focused tests, full Rust suite, Windows cross-check, changed-file formatting, Clippy warning ceiling and workflow contracts. Request independent review, resolve material findings with regressions, push to main and confirm GitHub Windows-native tests. A short hosted test does not establish 24-hour hardware stability.
