//! Narrow, explicit C ABI boundary to the C++ rendering-side foundation.

use std::ffi::CStr;
use std::os::raw::c_char;

unsafe extern "C" {
    fn browser_renderer_name() -> *const c_char;
    fn browser_renderer_self_check() -> i32;
}

pub fn renderer_name() -> String {
    // SAFETY: the C++ function returns a pointer to a static, NUL-terminated string.
    let ptr = unsafe { browser_renderer_name() };
    if ptr.is_null() {
        return "unavailable".to_owned();
    }
    // SAFETY: the pointer contract above guarantees a valid static C string.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

pub fn self_check() -> bool {
    // SAFETY: this function has no arguments and does not retain any Rust pointers.
    unsafe { browser_renderer_self_check() == 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_bridge_is_connected() {
        assert!(renderer_name().contains("C++"));
        assert!(self_check());
    }
}
