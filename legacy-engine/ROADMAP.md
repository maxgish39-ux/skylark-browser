# Roadmap

- [x] Phase 1: project structure and Rust/C++ ABI bridge
- [x] Phase 2: native window shell
- [x] Phase 3: HTTP/HTTPS fetching
- [x] Phase 4: experimental HTML parser and DOM
- [x] Phase 5: CSS declarations, selectors, cascade, inline and embedded styles
- [x] Phase 6: approximate block layout and text wrapping
- [x] Phase 7: paint commands and software rasterizer
- [x] Phase 8: CSS colors, scroll input, viewport-bounded rasterization, and basic links (source implemented; build not verified)
- [x] Phase 9: URL policy and approximate inherited foreground/font sizing (source implemented; build not verified)
- [x] Phase 10: editable address bar and keyboard URL submission (source implemented; build not verified)
- [x] Phase 11: shared inline text/link flow, per-word link hit-testing, and in-memory back/forward history (source implemented; build not verified)
- [x] Phase 12: limited inherited CSS properties, box geometry fixes, and inline whitespace/line-flow improvements (source implemented; build not verified)
- [ ] Phase 12: CSS and layout improvements
- [x] Phase 13: bounded image loading, decoding, and painting (source implemented; build not verified)
- [x] Phase 14: external stylesheets and resource handling (source implemented; build not verified)
- [x] Phase 15: browser usability, keyboard shortcuts, navigation controls, and error feedback (source implemented; build not verified)
- [x] Phase 16: evaluate JavaScript runtime and document integration architecture (decision documented; no JS support implemented)
- [ ] Phase 17: dedicated compilation, integration, runtime testing, and stabilization

Milestones indicate source work, not browser-standard compliance or verified functionality.


## Phase 12 — CSS and layout improvements (feature pass)

- [x] Propagate a limited inherited-property set through the computed style tree.
- [x] Add focused tests for inherited text styling and inline geometry.
- [x] Document box-model, whitespace, and bitmap-font limitations.
- [ ] Full CSS inheritance/computed values, margin collapsing, robust line-breaking, and layout invalidation remain future work.
- [ ] Build/test verification deferred to Phase 17.


## Phase 13 — image loading and painting

- [x] Resolve image sources against the final document URL and restrict requests to HTTP(S).
- [x] Bound image response bytes and decoded dimensions/pixel count.
- [x] Decode selected raster formats and paint through our software rasterizer.
- [x] Add deterministic source tests for URL resolution, invalid image input, image geometry, and raster output.
- [ ] Compilation, tests, and live website rendering remain unverified; full browser image behavior is not claimed.


## Phase 14 — External stylesheets (source implemented; unverified)
- Fetch linked CSS using bounded HTTP(S) requests and resolve hrefs against the final document URL.
- Combine embedded and linked CSS in document order and feed it through the custom CSS parser/cascade.
- Keep failures non-fatal and cap request count and response sizes.
- Limitations: no `@import`, CSS `url()` resources, full media evaluation, or standards-compliant CSS parsing.
- Verification: compilation and runtime tests remain pending.

## Phase 15 — Browser usability (source implemented; unverified)
- [x] Added clickable back/forward controls wired to the existing navigation intent loop.
- [x] Added Ctrl+L address focus/replace input, Ctrl+R reload, and retained Alt+Left/Right history shortcuts.
- [x] Added a visible navigation-error window for failed page fetches.
- [x] Added address normalization tests and documented current behavior.
- [ ] Full text editing/selection, in-window retry, non-blocking loading indicators, and document reflow on resize remain future work.
- [ ] Compilation, automated tests, and interactive browser behavior remain unverified.

## Phase 16 — JavaScript engine decision (completed as an architecture decision)
- [x] Compared Boa, QuickJS-NG, Duktape, and system-installed runtimes; selected Boa for the first optional Rust-native prototype.
- [x] Documented DOM/event integration, security/resource boundaries, licensing considerations, and staged implementation in `docs/JS_RUNTIME_DECISION.md`.
- [x] Confirmed no runtime dependency or script execution was added; custom HTML/CSS/layout/paint remain project-owned.
- [ ] Phase 17: compile and stabilize the current source before runtime integration. Build/test verification remains pending.

