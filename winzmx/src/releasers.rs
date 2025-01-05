use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{DeleteObject, HDC, HFONT, SaveDC, ReleaseDC, RestoreDC};


pub struct DeviceContext {
    pub window: HWND,
    pub context: HDC,
}
impl DeviceContext {
    pub fn new(window: HWND, context: HDC) -> Self {
        Self {
            window,
            context,
        }
    }

    #[allow(unused)]
    pub fn into_raw(mut self) -> (HWND, HDC) {
        let ret = (self.window, self.context);
        self.window = HWND::default();
        self.context = HDC::default();
        ret
    }
}
impl Drop for DeviceContext {
    fn drop(&mut self) {
        if self.window != HWND::default() && self.context != HDC::default() {
            unsafe { ReleaseDC(self.window, self.context) };
            self.window = HWND::default();
            self.context = HDC::default();
        }
    }
}

pub struct GdiFont(pub HFONT);
impl GdiFont {
    #[allow(unused)]
    pub fn into_raw(self) -> HFONT {
        self.0
    }
}
impl Drop for GdiFont {
    fn drop(&mut self) {
        if self.0 != HFONT::default() {
            let _ = unsafe { DeleteObject(self.0) };
            self.0 = HFONT::default();
        }
    }
}

pub struct ContextSaverRestorer {
    context: HDC,
    stack_value: i32,
}
impl ContextSaverRestorer {
    #[inline]
    pub fn new(context: HDC) -> Self {
        let stack_value = unsafe { SaveDC(context) };
        Self {
            context,
            stack_value,
        }
    }
}
impl Drop for ContextSaverRestorer {
    fn drop(&mut self) {
        if self.context != HDC::default() {
            let _ = unsafe { RestoreDC(self.context, self.stack_value) };
            self.context = HDC::default();
        }
    }
}
