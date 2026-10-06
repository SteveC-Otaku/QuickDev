// Tauri MockRuntime still links native menus. Validate the test executable's
// Common Controls activation context rather than relying on Python/VS to load it.
#[cfg(windows)]
#[test]
fn native_menu_runtime_resolves_common_controls_v6() {
    use std::ffi::{c_char, c_void};
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    let name: Vec<u16> = "comctl32.dll\0".encode_utf16().collect();
    unsafe {
        let module = LoadLibraryW(name.as_ptr());
        assert!(!module.is_null());
        let dialog = GetProcAddress(module, c"TaskDialogIndirect".as_ptr());
        let subclass = GetProcAddress(module, c"DefSubclassProc".as_ptr());
        FreeLibrary(module);
        assert!(!dialog.is_null(), "Common Controls v6 manifest is required");
        assert!(!subclass.is_null(), "native menu subclass API is required");
    }
}
