use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use otto_core::{MAX_TASK_TEXT_BYTES, Rect, scale};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DeleteObject, FW_NORMAL, HBRUSH, HDC, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, SetBkColor,
    SetTextColor,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{EM_SETLIMITTEXT, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, ES_AUTOHSCROLL, ES_LEFT,
    GWLP_USERDATA, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, HWND_TOPMOST,
    IDC_IBEAM, IsWindow, LoadCursorW, PostMessageW, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WM_CHAR,
    WM_CTLCOLOREDIT, WM_KEYDOWN, WM_KILLFOCUS, WM_NCCREATE, WM_NCDESTROY, WM_SETFONT, WNDCLASSEXW,
    WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::{HSTRING, PCWSTR, w};

const POPUP_CLASS: PCWSTR = w!("OttoQuickCapture");
const EDIT_SUBCLASS_ID: usize = 1;
const EDIT_LINE_HEIGHT_POINTS: i32 = 18;
const FONT_POINTS: i32 = 13;
const TEXT_COLOR: COLORREF = COLORREF(0x00F5_F5F7);
const BACKGROUND_COLOR: COLORREF = COLORREF(0x001A_1514);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSubmission {
    pub text: String,
    pub keep_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureEvent {
    Submitted(CaptureSubmission),
    Cancelled,
}

struct Shared {
    owner: HWND,
    notify_message: u32,
    popup: Cell<HWND>,
    edit: Cell<HWND>,
    brush: Cell<HBRUSH>,
    font: Cell<HFONT>,
    font_dpi: Cell<u32>,
    last_rect: Cell<Rect>,
    previous_foreground: Cell<HWND>,
    visible: Cell<bool>,
    allow_keep_open: Cell<bool>,
    suppress_blur_cancel: Cell<bool>,
    event: RefCell<Option<CaptureEvent>>,
}

impl Shared {
    fn hide(&self, restore_focus: bool) {
        if !self.visible.replace(false) {
            return;
        }
        self.last_rect.set(Rect::default());
        let prev_guard = self.suppress_blur_cancel.replace(true);
        unsafe {
            let _ = ShowWindow(self.popup.get(), SW_HIDE);
            let previous = self.previous_foreground.replace(HWND::default());
            if restore_focus && !previous.is_invalid() && IsWindow(Some(previous)).as_bool() {
                let _ = SetForegroundWindow(previous);
            }
        }
        self.suppress_blur_cancel.set(prev_guard);
    }

    fn post_notify(&self) {
        if self.owner.is_invalid() || self.notify_message == 0 {
            return;
        }
        unsafe {
            let _ = PostMessageW(Some(self.owner), self.notify_message, WPARAM(0), LPARAM(0));
        }
    }

    fn cancel(&self, restore_focus: bool) {
        if !self.visible.get() {
            return;
        }
        if let Ok(mut slot) = self.event.try_borrow_mut()
            && slot.is_none()
        {
            *slot = Some(CaptureEvent::Cancelled);
        }
        self.hide(restore_focus);
        self.post_notify();
    }

    fn submit(&self) {
        let edit = self.edit.get();
        let text = unsafe {
            let length = usize::try_from(GetWindowTextLengthW(edit)).unwrap_or(0);
            let mut buffer = vec![0u16; length + 1];
            let copied = usize::try_from(GetWindowTextW(edit, &mut buffer)).unwrap_or(0);
            String::from_utf16_lossy(&buffer[..copied.min(length)])
        };
        let keep_open =
            self.allow_keep_open.get() && unsafe { GetKeyState(i32::from(VK_SHIFT.0)) } < 0;
        if let Ok(mut slot) = self.event.try_borrow_mut() {
            *slot = Some(CaptureEvent::Submitted(CaptureSubmission {
                text,
                keep_open,
            }));
        }
        if keep_open {
            unsafe {
                let _ = SetWindowTextW(edit, w!(""));
            }
        } else {
            self.hide(true);
        }
        self.post_notify();
    }

    fn apply_font(&self, dpi: u32) {
        if self.font_dpi.get() == dpi {
            return;
        }
        let font = unsafe {
            CreateFontW(
                -scale(FONT_POINTS, dpi),
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI Variable Text"),
            )
        };
        if font.is_invalid() {
            return;
        }
        unsafe {
            SendMessageW(
                self.edit.get(),
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            let old = self.font.replace(font);
            if !old.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
        }
        self.font_dpi.set(dpi);
    }

    fn layout_edit(&self, rect: Rect, dpi: u32) {
        let edit_height = scale(EDIT_LINE_HEIGHT_POINTS, dpi).min(rect.height).max(1);
        let edit_y = ((rect.height - edit_height) / 2).max(0);
        unsafe {
            let _ = SetWindowPos(
                self.edit.get(),
                None,
                0,
                edit_y,
                rect.width,
                edit_height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub struct QuickCaptureInput {
    shared: *mut Shared,
}

impl QuickCaptureInput {
    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }

    #[must_use]
    pub fn create(owner: isize, notify_message: u32) -> Option<Self> {
        let instance = unsafe { GetModuleHandleW(None) }.ok()?.into();
        let brush = unsafe { CreateSolidBrush(BACKGROUND_COLOR) };
        if brush.is_invalid() {
            return None;
        }
        let shared = Box::into_raw(Box::new(Shared {
            owner: HWND(owner as *mut c_void),
            notify_message,
            popup: Cell::new(HWND::default()),
            edit: Cell::new(HWND::default()),
            brush: Cell::new(brush),
            font: Cell::new(HFONT::default()),
            font_dpi: Cell::new(0),
            last_rect: Cell::new(Rect::default()),
            previous_foreground: Cell::new(HWND::default()),
            visible: Cell::new(false),
            allow_keep_open: Cell::new(true),
            suppress_blur_cancel: Cell::new(false),
            event: RefCell::new(None),
        }));
        let input = Self { shared };
        unsafe {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(popup_proc),
                hInstance: instance,
                hCursor: LoadCursorW(None, IDC_IBEAM).unwrap_or_default(),
                hbrBackground: brush,
                lpszClassName: POPUP_CLASS,
                ..Default::default()
            };
            RegisterClassExW(&class);
            let popup = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                POPUP_CLASS,
                w!("Quick Capture"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                Some(shared.cast::<c_void>()),
            )
            .ok()?;
            input.shared().popup.set(popup);
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE_EDIT,
                0,
                0,
                0,
                0,
                Some(popup),
                None,
                Some(instance),
                None,
            )
            .ok()?;
            input.shared().edit.set(edit);
            SendMessageW(
                edit,
                EM_SETLIMITTEXT,
                Some(WPARAM(MAX_TASK_TEXT_BYTES)),
                Some(LPARAM(0)),
            );
            let installed =
                SetWindowSubclass(edit, Some(edit_proc), EDIT_SUBCLASS_ID, shared as usize);
            if !installed.as_bool() {
                return None;
            }
        }
        Some(input)
    }

    pub fn show_inline(&self, rect: Rect, initial_text: &str, allow_keep_open: bool, dpi: u32) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let shared = self.shared();
        shared.allow_keep_open.set(allow_keep_open);
        shared.apply_font(dpi);
        shared.layout_edit(rect, dpi);
        if let Ok(mut slot) = shared.event.try_borrow_mut() {
            *slot = None;
        }
        let wide = HSTRING::from(initial_text);
        let end = initial_text.encode_utf16().count();
        let prev_guard = shared.suppress_blur_cancel.replace(true);
        unsafe {
            let _ = SetWindowTextW(shared.edit.get(), &wide);
            SendMessageW(
                shared.edit.get(),
                EM_SETSEL,
                Some(WPARAM(end)),
                Some(LPARAM(end as isize)),
            );
            if !shared.visible.get() {
                let foreground = GetForegroundWindow();
                if foreground != shared.popup.get() {
                    shared.previous_foreground.set(foreground);
                }
            }
            shared.visible.set(true);
            shared.last_rect.set(rect);
            let _ = SetWindowPos(
                shared.popup.get(),
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(shared.popup.get());
            let _ = SetFocus(Some(shared.edit.get()));
        }
        shared.suppress_blur_cancel.set(prev_guard);
    }

    pub fn reposition(&self, rect: Rect, dpi: u32) {
        let shared = self.shared();
        if !shared.visible.get() || rect.width <= 0 || rect.height <= 0 {
            return;
        }
        shared.apply_font(dpi);
        if shared.last_rect.get() == rect {
            self.raise();
        } else {
            shared.layout_edit(rect, dpi);
            shared.last_rect.set(rect);
            unsafe {
                let _ = SetWindowPos(
                    shared.popup.get(),
                    Some(HWND_TOPMOST),
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    SWP_NOACTIVATE,
                );
            }
        }
    }

    pub fn raise(&self) {
        let shared = self.shared();
        if !shared.visible.get() {
            return;
        }
        unsafe {
            let _ = SetWindowPos(
                shared.popup.get(),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    pub fn hide(&self, restore_focus: bool) {
        self.shared().hide(restore_focus);
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.shared().visible.get()
    }

    #[must_use]
    pub fn take_event(&self) -> Option<CaptureEvent> {
        self.shared()
            .event
            .try_borrow_mut()
            .ok()
            .and_then(|mut slot| slot.take())
    }
}

impl Drop for QuickCaptureInput {
    fn drop(&mut self) {
        unsafe {
            let shared = &*self.shared;
            shared.suppress_blur_cancel.set(true);
            shared.visible.set(false);
            let edit = shared.edit.get();
            if !edit.is_invalid() {
                let _ = RemoveWindowSubclass(edit, Some(edit_proc), EDIT_SUBCLASS_ID);
            }
            let popup = shared.popup.get();
            if !popup.is_invalid() {
                let _ = DestroyWindow(popup);
            }
            let font = shared.font.get();
            if !font.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
            let brush = shared.brush.get();
            if !brush.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            drop(Box::from_raw(self.shared));
        }
    }
}

const WINDOW_STYLE_EDIT: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE =
    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_LEFT as u32);

unsafe extern "system" fn popup_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            if !create.is_null() {
                let params = unsafe { (*create).lpCreateParams };
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, params as isize) };
            }
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut c_void);
            let ptr = unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
            } as *const Shared;
            if !ptr.is_null() {
                let shared = unsafe { &*ptr };
                unsafe {
                    SetTextColor(hdc, TEXT_COLOR);
                    SetBkColor(hdc, BACKGROUND_COLOR);
                }
                return LRESULT(shared.brush.get().0 as isize);
            }
        }
        WM_NCDESTROY => {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference: usize,
) -> LRESULT {
    let shared = reference as *const Shared;
    if !shared.is_null() {
        let shared = unsafe { &*shared };
        match message {
            WM_KEYDOWN if wparam.0 == usize::from(VK_RETURN.0) => {
                shared.submit();
                return LRESULT(0);
            }
            WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => {
                shared.cancel(true);
                return LRESULT(0);
            }
            WM_CHAR if wparam.0 == 13 || wparam.0 == 27 => return LRESULT(0),
            WM_KILLFOCUS if !shared.suppress_blur_cancel.get() => shared.cancel(false),
            _ => {}
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use otto_core::Rect;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_RETURN};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageW, WM_KEYDOWN, WM_KILLFOCUS, WM_SETTEXT,
    };
    use windows::core::HSTRING;

    use super::{CaptureEvent, CaptureSubmission, QuickCaptureInput};

    const INLINE_RECT: Rect = Rect {
        x: 420,
        y: 42,
        width: 288,
        height: 20,
    };

    fn type_text(input: &QuickCaptureInput, text: &str) {
        let wide = HSTRING::from(text);
        unsafe {
            SendMessageW(
                input.shared().edit.get(),
                WM_SETTEXT,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
        }
    }

    fn press(input: &QuickCaptureInput, key: u16) {
        unsafe {
            SendMessageW(
                input.shared().edit.get(),
                WM_KEYDOWN,
                Some(WPARAM(usize::from(key))),
                Some(LPARAM(0)),
            );
        }
    }

    #[test]
    fn enter_submits_the_typed_text_and_hides_the_inline_input() {
        let input = QuickCaptureInput::create(0, 0).expect("inline input");
        input.show_inline(INLINE_RECT, "", true, 96);
        assert!(input.is_visible());
        type_text(&input, "kirim invoice");
        press(&input, VK_RETURN.0);
        assert_eq!(
            input.take_event(),
            Some(CaptureEvent::Submitted(CaptureSubmission {
                text: String::from("kirim invoice"),
                keep_open: false,
            }))
        );
        assert!(!input.is_visible());
        assert_eq!(input.take_event(), None);
    }

    #[test]
    fn edit_prefills_initial_text_and_submits_updated_value() {
        let input = QuickCaptureInput::create(0, 0).expect("inline input");
        input.show_inline(INLINE_RECT, "catatan lama", false, 96);
        assert!(input.is_visible());
        press(&input, VK_RETURN.0);
        assert_eq!(
            input.take_event(),
            Some(CaptureEvent::Submitted(CaptureSubmission {
                text: String::from("catatan lama"),
                keep_open: false,
            }))
        );
        assert!(!input.is_visible());
    }

    #[test]
    fn escape_and_blur_emit_cancelled_events_and_hide() {
        let input = QuickCaptureInput::create(0, 0).expect("inline input");
        input.show_inline(INLINE_RECT, "tidak jadi", true, 96);
        press(&input, VK_ESCAPE.0);
        assert!(!input.is_visible());
        assert_eq!(input.take_event(), Some(CaptureEvent::Cancelled));

        input.show_inline(INLINE_RECT, "blur test", false, 96);
        unsafe {
            SendMessageW(
                input.shared().edit.get(),
                WM_KILLFOCUS,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }
        assert!(!input.is_visible());
        assert_eq!(input.take_event(), Some(CaptureEvent::Cancelled));
    }
}
