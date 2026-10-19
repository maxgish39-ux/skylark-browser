# Next Task — Phase 17: Compilation, Integration, and Stabilization

Before adding JavaScript, establish a reliable build baseline for the existing Rust/C++ project. Inspect and fix actual source/build errors, run `cargo check` and `cargo test`, build the C++ bridge, and record the exact toolchain/platform and results. Then smoke-test navigation, inline text/link hit-testing, history, images, external stylesheets, address controls, scrolling, and resizing. Do not claim a check passed unless it was run.

The Phase 16 JavaScript runtime decision is in `docs/JS_RUNTIME_DECISION.md`. Boa is the preferred candidate for a later optional prototype, but no JavaScript runtime or page-script support is currently integrated. Stabilize first; then implement script raw-text parsing, bounded runtime evaluation, and DOM/event bindings as separate measured steps.
