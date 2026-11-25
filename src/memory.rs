//! glibc allocator tuning. With a dozen worker threads (downloads, image
//! decoding, data, market) glibc gives each its own heap arena, and memory
//! freed there stays reserved instead of returning to the OS: browsing the
//! inventory measured 252 MB with default arenas and 192 MB with two.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod glibc {
    use std::ffi::c_int;

    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
        fn malloc_trim(pad: usize) -> c_int;
    }

    /// `M_ARENA_MAX` from glibc's malloc.h.
    const M_ARENA_MAX: c_int = -8;

    pub fn limit_arenas() {
        // SAFETY: plain allocator setting, called once before other threads start.
        unsafe { mallopt(M_ARENA_MAX, 2) };
    }

    pub fn trim() {
        // SAFETY: releases free heap pages back to the OS; no other effect.
        unsafe { malloc_trim(0) };
    }
}

/// Call first thing in `main`, before any thread is spawned.
pub fn limit_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    glibc::limit_arenas();
}

/// Return freed heap to the OS, e.g. after parsing a large download.
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    glibc::trim();
}
