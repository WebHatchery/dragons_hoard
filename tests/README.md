# Integration regression matrix

`legacy.rs` is the crate-level integration harness. It mounts the former
source test modules from `tests/legacy/src/` through deliberate public seams in
the library, so the production crate has no `cfg(test)` modules or test-only
helpers.

The matrix intentionally keeps focused cases for distinct rule boundaries,
thresholds, seeded outcomes, malformed data, persistence migrations, and UI
reachability. It is not collapsed to an arbitrary five examples per feature:
those cases encode separate regressions and are the justified exception to the
heuristic in CODE_STANDARDS §11. Run it with:

```powershell
cargo test --test legacy
```
