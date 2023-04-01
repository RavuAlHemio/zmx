use once_cell::sync::Lazy;
use windows::core::PCSTR;
use windows::Win32::Foundation::{HMODULE, HWND, MAX_PATH};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK};

use crate::show_message_box;
use crate::string_holder::StringHolder;


pub static USER32: Lazy<HMODULE> = Lazy::new(|| {
    match load_system_dll("user32.dll") {
        Ok(dll) => dll,
        Err(e) => {
            let text = format!("failed to load user32.dll: {}", e);
            show_message_box(None, &text, MB_ICONERROR | MB_OK);
            panic!("{}", text);
        },
    }
});
pub static GET_DPI_FOR_WINDOW: Lazy<Option<unsafe extern "system" fn(HWND) -> u32>> = Lazy::new(|| {
    get_symbol(*USER32, "GetDpiForWindow")
        .map(|f| unsafe { std::mem::transmute(f) })
});


fn load_system_dll(name: &str) -> Result<HMODULE, windows::core::Error> {
    let mut file_path = get_system_directory();
    if !file_path.ends_with("\\") {
        file_path.append_str("\\");
    }
    file_path.append_str(name);

    unsafe {
        LoadLibraryW(file_path.as_pcwstr())
    }
}

/// Obtains a symbol from the given module.
///
/// Use [`std::mem::transmute`] to convert it to its actual type.
fn get_symbol(module: HMODULE, name: &str) -> Option<unsafe extern "system" fn() -> isize> {
    let mut name_buf = Vec::from(name.as_bytes());
    name_buf.push(0x00);
    // quite possibly the only Windows function without a Unicode variant
    unsafe {
        GetProcAddress(module, PCSTR(name_buf.as_ptr()))
    }
}


fn get_system_directory() -> StringHolder {
    // Windows paths: "D:\<path>\0" where "<path>" is up to 256 chars
    // MAX_PATH is 260, which means "\0" is included in the count
    let max_path: usize = MAX_PATH.try_into().unwrap();
    let mut buf = vec![0u16; max_path];
    let result = unsafe {
        GetSystemDirectoryW(Some(buf.as_mut_slice()))
    };
    if result == 0 {
        let text = format!("failed to obtain system directory: {}", windows::core::Error::from_win32());
        show_message_box(None, &text, MB_ICONERROR | MB_OK);
        panic!("{}", text);
    }
    StringHolder::from_slice_nul_terminated(&buf)
}
