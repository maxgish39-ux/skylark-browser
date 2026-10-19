# Architecture

- `src/main.rs`: CLI orchestration and DOM/CSS/layout diagnostics.
- `src/network.rs`: HTTP/HTTPS URL policy, request transport, and bounded response body.
- `src/html.rs`: experimental HTML tokenizer, tree builder, and DOM helpers.
- `src/css.rs`: CSS tokenization, declarations, selector matching, and computed author styles.
- `src/layout.rs`: approximate block flow, text wrapping, and layout boxes.
- `src/window.rs`: diagnostic native window and pixel buffer.
- `src/render_bridge.rs`: Rust wrapper for the C ABI.
- `cpp/include/render_bridge.h`, `cpp/src/render_bridge.cpp`: project-owned C++ bridge.

Pipeline: `URL -> response -> HTML -> DOM -> CSS -> computed styles -> layout boxes -> paint commands -> window`.
Phase 6 reaches layout boxes but does not yet rasterize or display fetched pages in the native window.
