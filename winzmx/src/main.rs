mod string_holder;


use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::mem::size_of_val;
use std::os::windows::prelude::OsStringExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;

use libzmx::{ZipCentralDirectoryEntry, zip_get_files};
use once_cell::sync::OnceCell;
use windows::w;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, CW_USEDEFAULT, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW,
    IDI_APPLICATION, LoadCursorW, LoadIconW, MB_ICONERROR, MB_OK, MESSAGEBOX_RESULT,
    MESSAGEBOX_STYLE, MessageBoxW, MSG, PostQuitMessage, RegisterClassExW, ShowWindow,
    SW_SHOWDEFAULT, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WM_SIZE, WNDCLASSEXW,
    WNDCLASS_STYLES, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

use crate::string_holder::StringHolder;


/// The current state of the application.
struct State {
    pub zip_file: File,
    pub file_path: PathBuf,
    pub entries: Vec<ZipCentralDirectoryEntry>,

    pub main_window: HWND,
    pub list_box: HWND,
    pub button: HWND,
}


static STATE: OnceCell<Mutex<State>> = OnceCell::new();


extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    println!("got message {}", msg);
    if msg == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        LRESULT(0)
    } else if msg == WM_SIZE {
        let width = (((lparam.0 as usize) >> 0) & 0xFFFF) as u16;
        let height = (((lparam.0 as usize) >> 16) & 0xFFFF) as u16;
        handle_window_resized(hwnd, width, height);
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

fn handle_window_resized(hwnd: HWND, width: u16, height: u16) {

}

fn show_message_box(parent_hwnd: Option<HWND>, text: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let text_sh = StringHolder::from_str(text);
    let parent_hwnd_real = parent_hwnd.unwrap_or(HWND::default());
    unsafe {
        MessageBoxW(
            parent_hwnd_real,
            text_sh.as_pcwstr(),
            w!("WinZMX"),
            style,
        )
    }
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

    // open file
    let zip_file_res = File::options()
        .read(true)
        .write(true)
        .append(false)
        .truncate(false)
        .open(&file_path);
    let zip_file = match zip_file_res {
        Ok(zf) => zf,
        Err(e) => {
            let text = format!("failed to open {}: {}", file_path.display(), e);
            show_message_box(None, &text, MB_ICONERROR | MB_OK);
            return ExitCode::FAILURE;
        },
    };

    let state = State {
        zip_file,
        file_path,
        entries: Vec::new(),
        main_window: HWND::default(),
        list_box: HWND::default(),
        button: HWND::default(),
    };

    if let Err(_) = STATE.set(Mutex::new(state)) {
        show_message_box(None, "unexpected situation: STATE already set", MB_ICONERROR | MB_OK);
        return ExitCode::FAILURE;
    }

    // read ZIP file
    {
        let mut state_guard = STATE.get().unwrap().lock().unwrap();
        match zip_get_files(&state_guard.zip_file) {
            Ok(mut ze) => {
                state_guard.entries.append(&mut ze);
            },
            Err(e) => {
                let text = format!("failed to list {} entries: {}", state_guard.file_path.display(), e);
                drop(state_guard);
                show_message_box(None, &text, MB_ICONERROR | MB_OK);
                return ExitCode::FAILURE;
            },
        };
    }

    let instance = unsafe { GetModuleHandleW(None) }
        .expect("failed to obtain my own instance");

    let main_window_class = StringHolder::from_str("WinZMX-MainWindow");

    // define a window class
    let cursor = match unsafe { LoadCursorW(None, IDC_ARROW) } {
        Ok(c) => c,
        Err(e) => {
            let error_message = format!("failed to obtain cursor: {}", e);
            show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
            return ExitCode::FAILURE;
        },
    };
    let icon = match unsafe { LoadIconW(None, IDI_APPLICATION) } {
        Ok(c) => c,
        Err(e) => {
            let error_message = format!("failed to obtain icon: {}", e);
            show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
            return ExitCode::FAILURE;
        },
    };

    let mut window_class = WNDCLASSEXW::default();
    window_class.cbSize = size_of_val(&window_class).try_into().unwrap();
    window_class.style = WNDCLASS_STYLES::default();
    window_class.lpfnWndProc = Some(wnd_proc);
    window_class.hInstance = instance;
    window_class.hIcon = icon;
    window_class.hCursor = cursor;
    window_class.hbrBackground = HBRUSH(isize::try_from(COLOR_WINDOW.0).unwrap() + 1);
    window_class.lpszClassName = main_window_class.as_pcwstr();

    let window_class_atom = unsafe { RegisterClassExW(&window_class) };
    if window_class_atom == 0 {
        let error_message = format!("failed to register window class: {}", windows::core::Error::from_win32());
        show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
        return ExitCode::FAILURE;
    }

    // create the window
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            main_window_class.as_pcwstr(),
            w!("WinZMX"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            instance,
            None,
        )
    };
    if window == HWND::default() {
        let error_message = format!("failed to create window: {}", windows::core::Error::from_win32());
        show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
        return ExitCode::FAILURE;
    }

    // show the window
    unsafe {
        ShowWindow(window, SW_SHOWDEFAULT)
    };

    // pump messages
    loop {
        let mut msg = MSG::default();
        let message_value = unsafe {
            GetMessageW(
                &mut msg,
                None,
                0,
                0,
            )
        };
        if message_value.0 == 0 {
            // WM_QUIT; break out of loop
            break;
        }
        if message_value.0 == -1 {
            let error_message = format!("failed to obtain message: {}", windows::core::Error::from_win32());
            show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
            return ExitCode::FAILURE;
        }

        // regular message
        unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }

    ExitCode::SUCCESS
}
