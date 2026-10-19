# Dependencies

- `minifb`: cross-platform diagnostic window and pixel buffer.
- `font8x8`: bitmap font for diagnostic UI text.
- `url`: URL parsing and normalization.
- `ureq`: HTTP/TLS transport, not a browser engine.
- `image`: selected raster-format decoding only; HTML, CSS, layout, and painting remain project-owned.
- `cc` (build dependency): compiles the project's own C++ bridge.

The HTML parser and CSS tokenizer/parser/cascade use only the Rust standard library.


## Phase 14 resource policy
External stylesheets use the existing `ureq` HTTP/TLS transport and `url` crate for URL joining; no browser engine or new dependency was added. Requests are bounded to 16 unique stylesheets, 512 KiB per stylesheet, and 4 MiB aggregate accepted stylesheet content per document.


## Phase 16 JavaScript runtime decision
No JavaScript dependency has been added. Boa (`boa_engine`) is the preferred candidate for a future optional Rust-native prototype; its dependency/build and memory costs must be measured on target hardware. See `JS_RUNTIME_DECISION.md` for alternatives, license considerations, security boundaries, and integration stages.
