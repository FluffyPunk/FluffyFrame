//! Windows release builds are GUI-subsystem apps (no console window of their
//! own). Started from a terminal, they borrow that terminal's console so log
//! lines still print there; started from Explorer there is none to borrow and
//! nothing changes.

/// Call first thing in `main`, before the logger starts writing to stderr.
pub fn attach_parent() {
    #[cfg(all(windows, not(debug_assertions)))]
    win::attach_parent();
}

#[cfg(all(windows, not(debug_assertions)))]
mod win {
    use std::ffi::c_void;

    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(std_handle: u32) -> *mut c_void;
    }

    pub fn attach_parent() {
        // SAFETY: plain Win32 calls with constant arguments.
        unsafe {
            // Output redirected to a file or pipe already has a handle: keep it.
            let stderr = GetStdHandle(STD_ERROR_HANDLE);
            if stderr.is_null() || stderr as isize == -1 {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }
}
