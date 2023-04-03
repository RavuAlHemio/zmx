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

use libzmx::{
    ZipCentralDirectoryEntry, best_effort_decode, zip_get_files, zip_make_executable,
    zip_make_not_executable,
};
use once_cell::sync::OnceCell;
use windows::w;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{FALSE, HMODULE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_CENTER, BS_PUSHBUTTON, CreateWindowExW, CW_USEDEFAULT, DefWindowProcW,
    DispatchMessageW, GetMessageW, GetWindowRect, HWND_TOP, IDC_ARROW, IDI_APPLICATION,
    IsDialogMessageW, LB_ADDSTRING, LB_GETSELCOUNT, LB_GETSELITEMS, LB_RESETCONTENT, LBN_SELCHANGE,
    LBS_EXTENDEDSEL, LBS_NOTIFY, LoadCursorW, LoadIconW, MB_ICONERROR, MB_OK, MESSAGEBOX_RESULT,
    MESSAGEBOX_STYLE, MessageBoxW, MoveWindow, MSG, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetWindowPos, SET_WINDOW_POS_FLAGS, SetWindowTextW, ShowWindow, SW_SHOW, SW_SHOWDEFAULT,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_CREATE, WM_DESTROY,
    WM_DPICHANGED, WM_SETFONT, WM_SIZE, WNDCLASSEXW, WNDCLASS_STYLES, WS_BORDER, WS_CHILD,
    WS_DISABLED, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VSCROLL,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};

use crate::graphics::{get_system_font, RectExt, Scaler};
use crate::string_holder::StringHolder;


const CHECKBOX_EMPTY: char = '\u{2610}';
const CHECKBOX_TICKED: char = '\u{2611}';


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
    } else if msg == WM_COMMAND {
        let mut state_guard = STATE.get().unwrap().lock().unwrap();
        let notif_code = ((wparam.0 >> 16) & 0xFFFF) as u32;
        if lparam.0 == state_guard.list_box.0 {
            // it's the list box
            if notif_code == LBN_SELCHANGE {
                // alright then
                handle_list_selection_changed(&mut *state_guard);
            }
        } else if lparam.0 == state_guard.button.0 {
            // it's the button
            if notif_code == BN_CLICKED {
                handle_button_clicked(&mut *state_guard);
            }
        }
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
            WINDOW_STYLE((LBS_NOTIFY | LBS_EXTENDEDSEL) as u32) | WS_BORDER | WS_CHILD | WS_TABSTOP | WS_VSCROLL,
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
    populate_list_box_from_entries(state);
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

fn handle_list_selection_changed(state: &mut State) {
    // activate or deactivate our button

    // which items are selected?
    let sel_count = unsafe { SendMessageW(state.list_box, LB_GETSELCOUNT, WPARAM(0), LPARAM(0)) };
    if sel_count.0 == 0 {
        // disable the button and jump out
        unsafe { EnableWindow(state.button, FALSE) };
        return;
    }

    let mut selected_buf = vec![0u32; sel_count.0 as usize];
    unsafe { SendMessageW(state.list_box, LB_GETSELITEMS, WPARAM(sel_count.0 as usize), LPARAM(selected_buf.as_mut_ptr() as isize)) };

    // what type of items are selected?
    let mut all_executable = true;
    let mut all_not_executable = true;
    for index_u32 in selected_buf {
        let index: usize = index_u32.try_into().unwrap();
        if state.entries[index].is_executable() {
            all_not_executable = false;
        } else {
            all_executable = false;
        }
    }

    if all_executable {
        unsafe { SetWindowTextW(state.button, w!("make non-&executable")) };
        unsafe { EnableWindow(state.button, TRUE) };
    } else if all_not_executable {
        unsafe { SetWindowTextW(state.button, w!("make &executable")) };
        unsafe { EnableWindow(state.button, TRUE) };
    } else {
        unsafe { EnableWindow(state.button, FALSE) };
    }
}

fn handle_button_clicked(state: &mut State) {
    // which items are selected?
    let sel_count = unsafe { SendMessageW(state.list_box, LB_GETSELCOUNT, WPARAM(0), LPARAM(0)) };
    if sel_count.0 == 0 {
        // not much to do here
        return;
    }

    let mut selected_buf = vec![0u32; sel_count.0 as usize];
    unsafe { SendMessageW(state.list_box, LB_GETSELITEMS, WPARAM(sel_count.0 as usize), LPARAM(selected_buf.as_mut_ptr() as isize)) };

    let first_selected: usize = selected_buf[0].try_into().unwrap();
    let make_executable = !state.entries[first_selected].is_executable();

    for index_u32 in selected_buf {
        let index: usize = index_u32.try_into().unwrap();
        let entry = &state.entries[index];
        let file_name = best_effort_decode(&entry.entry.file_name);
        if make_executable {
            if let Err(e) = zip_make_executable(&mut state.zip_file, entry.offset) {
                let message = format!("failed to make {:?} ({}) executable:\r\n{}", file_name, entry.offset, e);
                show_message_box(Some(state.main_window), &message, MB_OK | MB_ICONERROR);
            }
        } else {
            if let Err(e) = zip_make_not_executable(&mut state.zip_file, entry.offset) {
                let message = format!("failed to make {:?} ({}) non-executable:\r\n{}", file_name, entry.offset, e);
                show_message_box(Some(state.main_window), &message, MB_OK | MB_ICONERROR);
            }
        }
    }

    // reload all entries
    unsafe { SendMessageW(state.list_box, LB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    let entries = match zip_get_files(&mut state.zip_file) {
        Ok(f) => f,
        Err(e) => {
            let message = format!("failed to obtain fresh list of ZIP entries:\r\n{}", e);
            show_message_box(Some(state.main_window), &message, MB_OK | MB_ICONERROR);
            return;
        },
    };
    state.entries = entries;
    state.entries.sort_unstable_by_key(|e| e.entry.file_name.clone());
    populate_list_box_from_entries(state);
}

fn populate_list_box_from_entries(state: &mut State) {
    for entry in &state.entries {
        let checkbox = if entry.is_executable() { CHECKBOX_TICKED } else { CHECKBOX_EMPTY };
        let entry_name = best_effort_decode(&entry.entry.file_name);
        let entry_text = format!("{} {}", checkbox, entry_name);
        let entry_text_holder = StringHolder::from_str(&entry_text);
        unsafe { SendMessageW(state.list_box, LB_ADDSTRING, WPARAM(0), LPARAM(entry_text_holder.as_ptr() as isize)) };
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

        // force alignment by making a local copy (necessary on x86_32)
        let file_aligned = ofnw.lpstrFile;
        PathBuf::from(OsString::from_wide(unsafe { file_aligned.as_wide() }))
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
                state_guard.entries.sort_unstable_by_key(|e| e.entry.file_name.clone());
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
