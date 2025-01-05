use std::ffi::c_void;
use std::mem::size_of_val;

use windows::Win32::Foundation::{HWND, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, GetDC, GetDeviceCaps, GetTextExtentPoint32W, GetTextMetricsW, HFONT,
    LOGPIXELSX, SelectObject, TEXTMETRICW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};

use crate::show_message_box;
use crate::dynamic_linking::GET_DPI_FOR_WINDOW;
use crate::releasers::{ContextSaverRestorer, DeviceContext, GdiFont};
use crate::string_holder::StringHolder;


/// Extensions to rectangular types.
pub trait RectExt {
    type BackingType;

    fn width(&self) -> Self::BackingType;
    fn height(&self) -> Self::BackingType;
}
impl RectExt for RECT {
    type BackingType = i32;

    #[inline] fn width(&self) -> i32 { self.right - self.left }
    #[inline] fn height(&self) -> i32 { self.bottom - self.top }
}


pub fn get_system_font(message_box_parent: Option<HWND>, dpi_scaling_factor: f64) -> Option<HFONT> {
    let mut ncm = NONCLIENTMETRICSW::default();
    ncm.cbSize = size_of_val(&ncm).try_into().unwrap();
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };

    ncm.lfMessageFont.lfHeight = ((ncm.lfMessageFont.lfHeight as f64) * dpi_scaling_factor) as i32;

    if let Err(error) = result {
        let text = format!("failed to obtain non-client metrics: {}", error);
        show_message_box(message_box_parent, &text, MB_ICONERROR | MB_OK);
        return None;
    }
    let raw_font = unsafe { CreateFontIndirectW(&ncm.lfMessageFont) };
    if raw_font.is_invalid() {
        show_message_box(message_box_parent, "failed to create font", MB_ICONERROR | MB_OK);
        return None;
    }
    Some(raw_font)
}


/// Scales measurements in windows, controls and dialog boxes.
pub struct Scaler {
    base_x: i32,
    base_y: i32,
    dpi_scaling_factor: f64,
}
impl Scaler {
    pub fn new_from_window(hwnd: HWND) -> Option<Self> {
        // https://learn.microsoft.com/en-us/previous-versions/ms997619(v=msdn.10)
        // https://learn.microsoft.com/en-us/previous-versions/windows/desktop/bb226818%28v=vs.85%29
        // https://stackoverflow.com/a/58689/679474
        let raw_font = get_system_font(Some(hwnd), 1.0)?;
        let font = GdiFont(raw_font);

        // obtain the device context
        let raw_dc = unsafe { GetDC(hwnd) };
        if raw_dc.is_invalid() {
            show_message_box(Some(hwnd), "failed to obtain device context", MB_ICONERROR | MB_OK);
            return None;
        }
        let dc = DeviceContext::new(hwnd, raw_dc);

        // get DPI for the main screen
        let main_screen_dpi = unsafe { GetDeviceCaps(dc.context, LOGPIXELSX) };

        // get DPI for the current screen, if possible
        let current_screen_dpi = if let Some(get_dpi_for_window) = *GET_DPI_FOR_WINDOW {
            unsafe { get_dpi_for_window(hwnd) as f64 }
        } else {
            main_screen_dpi as f64
        };
        let dpi_scaling_factor = current_screen_dpi / (main_screen_dpi as f64);

        let _save_context = ContextSaverRestorer::new(dc.context);

        // activate the font on the context
        let previous_font = unsafe { SelectObject(dc.context, font.0) };
        if previous_font.is_invalid() {
            show_message_box(Some(hwnd), "failed to activate font", MB_ICONERROR | MB_OK);
            return None;
        }

        let mut text_metrics = TEXTMETRICW::default();
        let result = unsafe {
            GetTextMetricsW(
                dc.context,
                &mut text_metrics,
            )
        };
        if !result.as_bool() {
            show_message_box(Some(hwnd), "failed to obtain font metrics", MB_ICONERROR | MB_OK);
            return None;
        }

        // canonical measurements are in dialog units (DLUs)
        // pixelX = (dluX * baseX) / 4
        // pixelY = (dluY * baseY) / 8
        //
        // baseY is the height of the font
        // baseX is the average width of a letter in the font
        //          (MS Q145994 does some rounding magic to it)
        let base_y = ((text_metrics.tmHeight as f64) * dpi_scaling_factor) as i32;
        let alphabet = StringHolder::from_str("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz");
        let mut size = SIZE::default();
        let result = unsafe {
            GetTextExtentPoint32W(
                dc.context,
                alphabet.as_slice(false),
                &mut size,
            )
        };
        if !result.as_bool() {
            show_message_box(Some(hwnd), "failed to obtain text extent", MB_ICONERROR | MB_OK);
            return None;
        }
        let base_x = ((((size.cx / 26 + 1) / 2) as f64) * dpi_scaling_factor) as i32;

        Some(Self {
            base_x,
            base_y,
            dpi_scaling_factor,
        })
    }

    #[inline]
    pub fn scale_x(&self, x_dlu: i32) -> i32 {
        (i64::from(x_dlu) * i64::from(self.base_x) / 4) as i32
    }

    #[inline]
    pub fn scale_y(&self, y_dlu: i32) -> i32 {
        (i64::from(y_dlu) * i64::from(self.base_y) / 8) as i32
    }

    #[inline]
    pub fn scale_xy(&self, x_dlu: i32, y_dlu: i32) -> (i32, i32) {
        (self.scale_x(x_dlu), self.scale_y(y_dlu))
    }

    #[allow(unused)]
    #[inline]
    pub fn scale_font_size(&self, size: i32) -> i32 {
        ((size as f64) * self.dpi_scaling_factor) as i32
    }

    #[inline]
    pub const fn dpi_scaling_factor(&self) -> f64 { self.dpi_scaling_factor }
}
