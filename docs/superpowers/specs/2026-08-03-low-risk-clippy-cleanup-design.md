# Low-risk Clippy cleanup design

## Goal

Reduce the repository's existing Rust and Clippy warning debt without changing capture behavior, PPTX output, packaging, or public interfaces. The warning ceiling must decrease to the new Linux CI result so future changes cannot restore the removed warnings.

## Scope

This batch may fix warnings whose resolution is local and mechanically verifiable:

- unused imports, variables, and unnecessary `mut` bindings;
- manual clamp expressions and same-type casts;
- needless borrows and returns;
- simple test-only loop, assertion, and allocation warnings;
- derivable implementations where the derived behavior is identical.

This batch will not:

- delete or globally suppress `dead_code`;
- change public APIs merely to satisfy a lint;
- restructure functions flagged for argument count or large error types;
- change capture selection, image processing, PPTX generation, or platform behavior;
- force the warning baseline to zero before the remaining warnings are understood.

## Implementation approach

Warnings will be handled in small groups by lint category. Before each group, the current warning is reproduced with structured Cargo JSON. The smallest local code change is then applied, followed by focused tests and a fresh warning count.

Conditional compilation will be preferred over underscore renaming when a symbol is genuinely platform-specific. Test-only warnings will be corrected in test code without weakening assertions. Existing `allow` attributes will not be broadened.

The changed-file rustfmt gate must check only the paths explicitly selected by Git. A changed crate root must not cause rustfmt to recurse into unchanged modules; the checker will use `skip_children=true`, backed by a module-tree regression fixture.

## Quality gates

The existing Clippy baseline script remains the source of truth. After local tests pass, GitHub CI supplies the authoritative Linux warning count. `.clippy-warning-baseline` will be reduced to that count, never increased as part of this cleanup.

Verification includes:

- `cargo test --all-targets --all-features`;
- all Shell and Ruby contract tests;
- Apple Silicon DMG end-to-end packaging on macOS;
- changed-file rustfmt validation;
- structured Clippy warning counting;
- Linux, Windows, and Apple Silicon checks in GitHub Actions.

## Success criteria

- The selected low-risk lint categories are reduced without adding suppressions.
- All existing behavior and regression tests remain green.
- The Linux warning baseline is lower than 110.
- The change is merged into and pushed from `main`.
- The resulting GitHub CI run succeeds on all four jobs.
