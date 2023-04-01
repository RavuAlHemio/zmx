mod dynamic_linking;
mod graphics;
mod releasers;
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
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{FALSE, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_CENTER, BS_PUSHBUTTON, CreateWindowExW, CW_USEDEFAULT, DefWindowProcW, DispatchMessageW,
    GetMessageW, GetWindowRect, HWND_TOP, IDC_ARROW, IDI_APPLICATION, IsDialogMessageW,
    LB_ADDSTRING, LBS_NOTIFY, LoadCursorW, LoadIconW, MB_ICONERROR, MB_OK, MESSAGEBOX_RESULT,
    MESSAGEBOX_STYLE, MessageBoxW, MoveWindow, MSG, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetWindowPos, SET_WINDOW_POS_FLAGS, ShowWindow, SW_SHOW, SW_SHOWDEFAULT, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_GETFONT, WM_SETFONT,
    WM_SIZE, WNDCLASSEXW, WNDCLASS_STYLES, WS_BORDER, WS_CHILD, WS_DISABLED, WS_OVERLAPPEDWINDOW,
    WS_TABSTOP, WS_VSCROLL,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

use crate::graphics::{get_system_font, RectExt, Scaler};
use crate::string_holder::StringHolder;


/// The current state of the application.
struct State {
    pub zip_file: File,
    pub file_path: PathBuf,
    pub entries: Vec<ZipCentralDirectoryEntry>,

    pub instance: HMODULE,
    pub main_window: HWND,
    pub list_box: HWND,
    pub button: HWND,
    pub needs_new_font: bool,
}


static STATE: OnceCell<Mutex<State>> = OnceCell::new();


extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        LRESULT(0)
    } else if msg == WM_CREATE {
        let mut state_guard = STATE.get().unwrap().lock().unwrap();
        handle_window_create(&mut *state_guard, hwnd);
        LRESULT(0)
    } else if msg == WM_SIZE {
        let width = (((lparam.0 as usize) >> 0) & 0xFFFF) as u16;
        let height = (((lparam.0 as usize) >> 16) & 0xFFFF) as u16;
        let mut state_guard = STATE.get().unwrap().lock().unwrap();
        handle_window_resized(&mut *state_guard, hwnd, width.into(), height.into());
        LRESULT(0)
    } else if msg == WM_DPICHANGED {
        {
            let mut state_guard = STATE.get().unwrap().lock().unwrap();
            state_guard.needs_new_font = true;
        }
        let rect = unsafe { (lparam.0 as *const RECT).as_ref() }.unwrap();
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOP,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SET_WINDOW_POS_FLAGS::default(),
            )
        };
        // this also leads to a WM_SIZE message, which triggers the resize logic
        LRESULT(0)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

fn handle_window_create(state: &mut State, hwnd: HWND) {
    // obtain the system font
    let system_font = match get_system_font(Some(hwnd), 1.0) {
        Some(sf) => sf,
        None => return, // error already output in message box
    };

    // create the list box
    let list_box = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LISTBOX"),
            PCWSTR::null(),
            WINDOW_STYLE(LBS_NOTIFY as u32) | WS_BORDER | WS_CHILD | WS_TABSTOP | WS_VSCROLL,
            0, 0,
            32, 32,
            hwnd,
            None,
            state.instance,
            None,
        )
    };
    if list_box == HWND::default() {
        let error_message = format!("failed to create list box: {}", windows::core::Error::from_win32());
        show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
        return;
    }
    state.list_box = list_box;
    let test_text = w!("THE FAKEST OF ALL ENTRIES");
    unsafe { SendMessageW(list_box, LB_ADDSTRING, WPARAM(0), LPARAM(test_text.0 as isize)) };
    unsafe { SendMessageW(list_box, WM_SETFONT, WPARAM(system_font.0 as usize), LPARAM(FALSE.0 as isize)) };
    unsafe { ShowWindow(list_box, SW_SHOW) };

    // create the enable/disable button
    let button = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("make non-&executable"),
            WINDOW_STYLE((BS_CENTER | BS_PUSHBUTTON) as u32) | WS_CHILD | WS_DISABLED | WS_TABSTOP,
            0, 0,
            32, 32,
            hwnd,
            None,
            state.instance,
            None,
        )
    };
    if button == HWND::default() {
        let error_message = format!("failed to create button: {}", windows::core::Error::from_win32());
        show_message_box(None, &error_message, MB_ICONERROR | MB_OK);
        return;
    }
    state.button = button;
    unsafe { SendMessageW(button, WM_SETFONT, WPARAM(system_font.0 as usize), LPARAM(FALSE.0 as isize)) };
    unsafe { ShowWindow(button, SW_SHOW) };

    let mut window_rect = RECT::default();
    let result = unsafe { GetWindowRect(hwnd, &mut window_rect) };
    if !result.as_bool() {
        show_message_box(None, "failed to get window rect", MB_ICONERROR | MB_OK);
        return;
    }

    handle_window_resized(
        state,
        hwnd,
        window_rect.right - window_rect.left,
        window_rect.bottom - window_rect.top,
    );
}

fn handle_window_resized(state: &mut State, hwnd: HWND, width: i32, height: i32) {
    if hwnd != state.main_window {
        return;
    }

    // prepare a scaler
    let scaler = match Scaler::new_from_window(hwnd) {
        Some(s) => s,
        None => return,
    };

    // default margin: 7 DLUs, padding: 4 DLUs
    let (margin_x, margin_y) = scaler.scale_xy(7, 7);
    let (_padding_x, padding_y) = scaler.scale_xy(4, 4);

    let mut new_font = HFONT(0);
    if state.needs_new_font {
        new_font = get_system_font(Some(hwnd), scaler.dpi_scaling_factor())
            .unwrap_or(HFONT(0));
        state.needs_new_font = false;
    }

    // button: width at least 50 DLUs, height 13 DLUs
    // we need more than 50 though
    let (button_min_width, button_height) = scaler.scale_xy(80, 13);
    unsafe {
        MoveWindow(
            state.button,
            width - (margin_x + button_min_width),
            height - (margin_y + button_height),
            button_min_width, button_height,
            true,
        )
    };
    if !new_font.is_invalid() {
        // also update the font
        unsafe { SendMessageW(state.button, WM_SETFONT, WPARAM(new_font.0 as usize), LPARAM(FALSE.0 as isize)) };
    }

    // fill the window with the list box
    unsafe {
        MoveWindow(
            state.list_box,
            margin_x, margin_y,
            width - 2*margin_x,
            height - (2*margin_y + button_height + padding_y),
            true,
        )
    };
    if !new_font.is_invalid() {
        // also update the font
        unsafe { SendMessageW(state.list_box, WM_SETFONT, WPARAM(new_font.0 as usize), LPARAM(FALSE.0 as isize)) };
    }
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

    let instance_res = unsafe { GetModuleHandleW(None) };
    let instance = match instance_res {
        Ok(i) => i,
        Err(e) => {
            let text = format!("failed to obtain my own instance: {}", e);
            show_message_box(None, &text, MB_ICONERROR | MB_OK);
            return ExitCode::FAILURE;
        },
    };

    let state = State {
        zip_file,
        file_path,
        entries: Vec::new(),
        instance,
        main_window: HWND::default(),
        list_box: HWND::default(),
        button: HWND::default(),
        needs_new_font: false,
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

    // store this as the main window
    {
        let mut state_guard = STATE.get().unwrap().lock().unwrap();
        state_guard.main_window = window;
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

        // dialog message?
        let is_dialog = unsafe { IsDialogMessageW(window, &msg) };
        if is_dialog.as_bool() {
            continue;
        }

        // regular message
        unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }

    ExitCode::SUCCESS
}
