#include "render_bridge.h"

// This is deliberately a small C ABI boundary. Future rendering APIs should
// exchange plain data structures and opaque handles, not Rust-owned objects.

extern "C" const char* browser_renderer_name(void) {
    return "C++ rendering foundation";
}

extern "C" int browser_renderer_self_check(void) {
    // Placeholder for a real renderer invariant check; the bridge itself is
    // exercised by the Rust integration test.
    return 1;
}
