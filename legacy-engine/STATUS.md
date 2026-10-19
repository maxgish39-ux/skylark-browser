# Project Status — Phase 16

## Implemented in source
- Rust HTML/CSS/layout/paint pipeline with narrow C++ C ABI bridge.
- HTTP/HTTPS fetching and URL resolution policy.
- Native resizable window, editable address bar, keyboard input, viewport scrolling, rasterized page display, and an explicit navigation-error window.
- Shared inline flow for text and anchor labels; words can share a line and wrap at the approximate content boundary.
- Per-word link rectangles for wrapped anchor labels; hit-testing checks the recorded rectangles rather than making a whole line clickable.
- In-memory navigation history model with back/forward traversal and forward-branch truncation.
- Alt+Left and Alt+Right shortcuts in the native page window; Phase 15 adds clickable back/forward controls, Ctrl+L address focus, and Ctrl+R reload.
- Deterministic source-level tests for inline flow, link hit testing, URL policy, history branching, and scroll clamping.

## Verification status
- The archive and source files were inspected and modified.
- Rust compilation, Rust tests, full runtime navigation, and interactive rendering were **not run**. Do not treat the added tests as passing.
- The C++ bridge was preserved, but no new C++ build verification was performed in this phase.

## Known limitations
- Inline formatting is deliberately approximate. Whitespace is tokenized, character widths are estimated, and long words may exceed the available line width.
- Bitmap glyphs remain fixed at 8x8. Font-size affects estimated layout geometry only; actual font metrics are not available.
- Link fragment rectangles are per word, not glyph-accurate. Rasterizer clipping and pointer coordinate mapping are approximate.
- Block margins/padding and mixed block/inline interactions remain limited.
- History is in-memory only. Back/forward index changes happen before the destination fetch is confirmed; a failed history fetch can leave the current session unable to continue in the current loop.
- No JavaScript execution, CSS `@import` or `url()` resources, tabs, downloads, bookmarks, or session restoration. Phase 16 documents Boa as the preferred prototype candidate only; no runtime dependency or JS API has been added.
- This is not a standards-compliant browser, and compatibility with real websites is not verified.


## Phase 12 — CSS and layout improvements

- Added propagation of supported inherited properties: color, font-family, font-size, font-style, font-weight, line-height, text-align, visibility, and white-space.
- Preserved non-inherited box properties as local declarations.
- Corrected source-level layout issues in inline word sizing and mutable content-height calculation.
- Kept content-box width/padding accounting explicit and added focused inheritance/geometry tests.
- Limitations: no full CSS computed-value algorithm, margin collapsing, accurate font metrics, or complete `white-space` behavior. Bitmap painting remains 8×8 glyphs.
- Verification: tests and compilation have not been run in this feature-first phase.


## Phase 13 — image resources (source implemented; unverified)

- HTTP(S) image URLs resolve against the final document URL.
- Image response bodies have a 5 MiB cap; decoded dimensions are capped at 2048×2048 and 4,194,304 pixels.
- PNG, JPEG, GIF, WebP, and BMP are enabled through the `image` crate; pixels are passed into our own paint commands and rasterizer.
- Relative URL resolution, corrupt-image rejection, intrinsic dimensions, and image rasterization have deterministic source tests.
- Not yet verified by compiling or running the application. SVG, `srcset`, animation, color profiles, and browser-complete image sizing remain unsupported.


## Phase 14 — external stylesheets (implemented in source; unverified)

- Linked stylesheets are discovered from `<link>` elements with a `stylesheet` token in `rel`; embedded `<style>` and external sheets are collected in document order.
- Relative URLs resolve against the final URL of the HTML document; only HTTP(S) destinations are allowed and URL fragments are removed before fetching.
- Non-2xx responses, unsupported content types, fetch errors, invalid URLs, and binary-looking bodies are ignored without aborting page rendering.
- Limits: 16 unique stylesheet requests, 512 KiB per stylesheet, 4 MiB aggregate accepted CSS. `media` supports only empty, `all`, or `screen`.
- CSS is processed by the existing custom parser/cascade; no complete CSS conformance is implied. `@import`, `url()` resources, `@media` evaluation, and asynchronous loading are not implemented.
- Verification: archive integrity can be checked, but `cargo check`, `cargo test`, and live navigation/rendering were not run.


## Phase 15 usability additions
- Clickable back/forward controls return navigation intents to the existing main navigation loop.
- Ctrl+L focuses and clears the address field for replacement input; Ctrl+R requests reload; Alt+Left/Right remain history shortcuts when the address field is not focused.
- Hostnames without an explicit scheme default to HTTPS. Explicit `http://` and `https://` input is preserved for the existing URL policy to validate.
- Fetch failures now have a visible error window. This error surface is blocking and closes on Escape/window close; retry-in-place and non-blocking loading indicators remain future work.
- Resizing changes the visible raster viewport and scrolling bounds, but the already-computed document layout does not reflow to the new width.
- Verification remains pending: no cargo build, test run, or live navigation was performed.


## Phase 16 — JavaScript runtime decision
- Selected Boa (`boa_engine`) as the first prototype candidate because the application orchestration and DOM are Rust-native. QuickJS-NG remains a smaller C-based alternative; Duktape and system-installed runtimes are not preferred.
- Added `docs/JS_RUNTIME_DECISION.md` covering runtime tradeoffs, license notes, security boundaries, a minimal DOM/event architecture, resource limits, and staged integration.
- No Cargo dependency changes and no JavaScript execution were added in this decision phase. Script raw-text parsing, stable DOM node IDs/mutation APIs, event/task scheduling, and robust invalidation are blockers before page scripts can be enabled.
- Verification: documentation/archive only; Rust compilation and tests remain unverified. Phase 17 should stabilize the existing source before adding Boa.
