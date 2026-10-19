# Phase 16 — JavaScript Runtime Decision

**Decision: prototype Boa (`boa_engine`) as the first JavaScript runtime candidate. Do not integrate it into page loading until Phase 17 has a verified baseline.** This is a selection for a constrained prototype, not a claim that Skylark will become a standards-compliant browser merely by adding a JS engine.

## Why Boa first

Skylark's orchestration, HTML tree, CSS, layout, paint pipeline, and native window are primarily Rust. Boa is an embeddable ECMAScript engine written in Rust, so a first prototype can be integrated without introducing another language boundary or adding JavaScript execution to the C++ renderer bridge. The upstream project describes itself as experimental and reports support for more than 90% of the current ECMAScript Test262 suite; that is language conformance, not browser API compatibility. Its project offers MIT or Unlicense licensing. See the upstream [Boa repository](https://github.com/boa-dev/boa) and [Boa documentation](https://boajs.dev/docs/intro).

Boa is the best fit for a staged Rust-first experiment, but it is not free of tradeoffs: its dependency graph is substantially larger than the current project's, build time and memory use must be measured on the target device, and a page-facing DOM/Web API layer must be implemented by Skylark. Do not infer a precise binary-size or memory figure from this decision document; collect measurements during the prototype.

## Alternatives considered

| Candidate | Advantages | Costs / reasons not selected first |
|---|---|---|
| **Boa (`boa_engine`)** | Rust-native; straightforward fit with current main application; actively developed; embeddable API | Experimental; dependency/build footprint; no automatic HTML DOM, browser event loop, Fetch, or complete Web APIs |
| QuickJS-NG | Small embeddable C engine; potentially compact runtime; mature embedding style | Adds a separate C runtime/build/update surface beside the existing narrow C++ bridge; host bindings and security limits still need custom work |
| Duktape | Small C footprint and simple embedding | Older ECMAScript target makes it a weaker foundation for modern websites |
| System-installed JavaScript runtime | May reduce bundled engine code in some environments | Unreliable availability across platforms; version mismatch and packaging issues; not suitable as the default portable build |

QuickJS-NG's upstream describes it as a small embeddable engine aiming to support modern ECMAScript: [QuickJS-NG repository](https://github.com/quickjs-ng/quickjs). Duktape is not recommended for the first integration. No candidate replaces the browser-owned HTML/CSS/layout/paint pipeline.

## Required architecture

Keep execution owned by a new Rust-side `script`/`js_runtime` module. Do not route JavaScript through the C++ rasterizer ABI.

1. **Script discovery:** after the HTML parser can correctly preserve script raw text, collect inline classic scripts and eligible external classic scripts in document order. Start with classic scripts only. Defer modules, `async`, `defer` scheduling, `document.write`, dynamic script insertion, and workers.
2. **Execution context:** create a fresh JS context per document navigation. Dispose of it when the document is replaced. Keep the host API deliberately small.
3. **DOM bridge:** expose wrapper objects and stable node IDs backed by the existing Rust DOM. Do not expose Rust pointers or references to JavaScript. All DOM mutations go through checked host methods. Initial scope: `document`, `document.getElementById`, a limited `querySelector`, basic node/text access, `textContent`, `setAttribute`, and a small event-listener API only after mutation invalidation exists.
4. **Events:** translate native click/navigation input into queued browser events. Dispatch handlers on the UI/main thread, never from the C++ paint callback. Queue navigation requests and apply them after the current script/event dispatch to avoid re-entrant document replacement.
5. **Rendering updates:** DOM mutation marks style/layout/paint dirty. Recompute using Skylark's own CSS, layout, and paint stages. JavaScript must not draw directly to the window.
6. **Network:** no generic host networking API. External scripts may only be fetched through the existing bounded HTTP(S) resource loader. No automatic `fetch`, XHR, WebSocket, or arbitrary scheme support in the first slice.

## Security and resource policy

Page scripts are untrusted. A JS interpreter is not, by itself, a complete sandbox.

- Do not register filesystem, process, shell, environment, arbitrary socket, or host inspection APIs.
- Restrict script URLs to HTTP(S), resolve relative URLs against the final document URL, and enforce per-resource and aggregate byte caps.
- Set a maximum script count, per-script source cap, total script-source cap, and an execution budget. Determine whether Boa exposes a reliable interrupt/timeout mechanism before claiming time limits; if not, do not run untrusted scripts in a way that can freeze the UI indefinitely. A separate worker process/thread is not automatically safe without a cancellable execution design.
- Bound DOM mutations and event dispatch work per turn; guard against recursive event dispatch and navigation loops.
- Treat all strings and attributes from the page as untrusted. Avoid executing event-handler attributes until they can be compiled and dispatched under the same policy.
- Do not claim isolation from memory exhaustion or CPU denial of service until quotas are enforced and tested.

## Cost and licensing

Boa is dual-licensed MIT or Unlicense upstream. Preserve license/notice files and review the licenses of transitive dependencies before distribution. The current `Cargo.toml` has no JS engine dependency; this phase intentionally does not add one. Dependency footprint, release binary size, cold-start latency, steady-state memory, and execution throughput must be measured on the actual target platforms before choosing default feature flags. Provide a `javascript` feature flag for early experiments so builds can disable it if resource costs are unacceptable.

## Staged integration plan

1. **Phase 17 baseline:** compile and test the current project, fix existing failures, and record platform/toolchain details before adding a large runtime dependency.
2. **Runtime spike:** add Boa behind an optional feature and test only arithmetic, strings, functions, exceptions, and context creation/disposal. Record binary/build/memory costs.
3. **Safe script extraction:** fix HTML raw-text handling for `<script>` first; add bounded inline script execution tests. No DOM globals yet.
4. **Minimal host DOM:** add stable node IDs, a small document API, mutation invalidation, and deterministic tests.
5. **Events and navigation:** add queued `click`/basic event dispatch and safe navigation requests.
6. **External scripts:** add bounded URL loading and ordered execution for classic scripts.
7. **Stabilize:** run unit tests, malicious-input/resource-limit tests, and real page smoke tests. Keep JavaScript explicitly experimental until results justify broader claims.

## Current project-specific blockers

- `src/html.rs` currently tokenizes tags generically; it must be taught HTML raw-text handling for `script` before reliable extraction of inline JavaScript containing `<` characters.
- The current DOM is a simple owned tree. It has no mutation API or stable node IDs, so exposing direct references to `Node` would be unsafe and fragile.
- There is no browser event loop, task queue, timer system, Promise job scheduling, or complete DOM event model.
- CSS/layout invalidation is not yet a robust incremental pipeline; mutations should initially trigger a full style/layout/paint rebuild.
- JavaScript support is **not implemented** in this phase. This document is an architecture decision only.

## References

- Boa project and license: https://github.com/boa-dev/boa
- Boa documentation: https://boajs.dev/docs/intro
- QuickJS-NG project: https://github.com/quickjs-ng/quickjs
