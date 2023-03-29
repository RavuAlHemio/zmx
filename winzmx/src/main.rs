use std::env;
use std::ffi::OsString;
use std::mem::size_of_val;
use std::os::windows::prelude::OsStringExt;
use std::path::PathBuf;
use std::process::ExitCode;

use windows::w;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::Win32::UI::Controls::Dialogs::{
    OFN_ALLOWMULTISELECT,

    GetOpenFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};


unsafe extern "system" fn hook_func(_hdlg: HWND, _ui_msg: u32, _wparam: WPARAM, _lparam: LPARAM) -> usize {
    0
}


fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().collect();
    let file_path = if args.len() == 1 {
        let mut file_name_buffer = vec![0u16; 32768];

        // show open file dialog
        let mut ofnw = OPENFILENAMEW::default();
        ofnw.lStructSize = size_of_val(&ofnw).try_into().unwrap();
        ofnw.lpstrFilter = w!("Zip archives (*.zip)\0*.zip\0All Files\0*.*\0\0");
        ofnw.lpstrFile = PWSTR::from_raw(file_name_buffer.as_mut_ptr());
        ofnw.nMaxFile = file_name_buffer.len().try_into().unwrap();
        ofnw.lpstrTitle = w!("WinZMX: Open");
        ofnw.Flags =
            OFN_ENABLESIZING | OFN_ALLOWMULTISELECT | OFN_EXPLORER | OFN_HIDEREADONLY
            | OFN_PATHMUSTEXIST
        ;
        ofnw.lpfnHook = Some(hook_func);
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

    ExitCode::SUCCESS
}
