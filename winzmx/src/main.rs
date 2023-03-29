use std::env;
use std::ffi::OsString;
use std::mem::size_of_val;
use std::os::windows::prelude::OsStringExt;
use std::path::PathBuf;
use std::process::ExitCode;

use windows::w;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, HCURSOR, HICON, MB_ICONERROR, MB_OK, MessageBoxW, WNDCLASSEXW, WNDCLASS_STYLES,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};


extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}


fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().collect();

    // find out which file we're trying to analyze
    let file_path = if args.len() == 1 {
        let mut file_name_buffer = vec![0u16; 32768];

        // show open file dialog
        let mut ofnw = OPENFILENAMEW::default();
        ofnw.lStructSize = size_of_val(&ofnw).try_into().unwrap();
        ofnw.lpstrFilter = w!("Zip archives (*.zip)\0*.zip\0All Files\0*.*\0\0");
        ofnw.lpstrFile = PWSTR::from_raw(file_name_buffer.as_mut_ptr());
        ofnw.nMaxFile = file_name_buffer.len().try_into().unwrap();
        ofnw.lpstrTitle = w!("WinZMX: Open");
        ofnw.Flags = OFN_ENABLESIZING | OFN_EXPLORER | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST;
        let success = unsafe { GetOpenFileNameW(&mut ofnw) };
        if !success.as_bool() {
            return ExitCode::FAILURE;
        }

        PathBuf::from(OsString::from_wide(unsafe { ofnw.lpstrFile.as_wide() }))
    } else if args.len() == 2 {
        // take ZIP path from argument
        PathBuf::from(&args[1])
    } else {
        unsafe {
            MessageBoxW(
                None,
                w!("Incorrect commandline arguments."),
                w!("WinZMX"),
                MB_OK | MB_ICONERROR,
            )
        };
        return ExitCode::FAILURE;
    };

    let instance = unsafe { GetModuleHandleW(None) }
        .expect("failed to obtain my own instance");

    // define a window class
    let mut window_class = WNDCLASSEXW::default();
    window_class.cbSize = size_of_val(&window_class).try_into().unwrap();
    window_class.style = WNDCLASS_STYLES::default();
    window_class.lpfnWndProc = Some(wnd_proc);
    window_class.hInstance = instance;
    window_class.hIcon = HICON::default();
    window_class.hCursor = HCURSOR::default();
    window_class.hbrBackground = HBRUSH(isize::try_from(COLOR_WINDOW.0).unwrap() + 1);
    window_class.lpszClassName = w!("WinZMX-MainWindow");

    // TODO

    ExitCode::SUCCESS
}
