use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use otto_core::{MAX_TASK_TEXT_BYTES, scale};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    DEFAULT_CHARSET, DeleteObject, FW_NORMAL, GetStockObject, HBRUSH, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, SetBkColor, SetTextColor, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::EM_SETLIMITTEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, ES_AUTOHSCROLL, ES_LEFT,
    GWLP_USERDATA, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, HWND_TOPMOST,
    IDC_IBEAM, IsWindow, LoadCursorW, PostMessageW, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WM_CHAR, WM_CTLCOLOREDIT,
    WM_KEYDOWN, WM_KILLFOCUS, WM_NCCREATE, WM_NCDESTROY, WM_SETFONT, WNDCLASSEXW, WS_CHILD,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const POPUP_CLASS: PCWSTR = w!("OttoQuickCapture");
const EDIT_SUBCLASS_ID: usize = 1;
const POPUP_WIDTH_POINTS: i32 = 360;
const POPUP_HEIGHT_POINTS: i32 = 40;
const POPUP_RADIUS_POINTS: i32 = 12;
const POPUP_PAD_POINTS: i32 = 16;
const EDIT_HEIGHT_POINTS: i32 = 20;
const FONT_POINTS: i32 = 15;
const POPUP_GAP_POINTS: i32 = 8;
const TEXT_COLOR: COLORREF = COLORREF(0x00F2_F2F2);
const BACKGROUND_COLOR: COLORREF = COLORREF(0x0000_0000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSubmission {
    pub text: String,
    pub keep_open: bool,
}

struct Shared {
    owner: HWND,
    notify_message: u32,
    popup: Cell<HWND>,
    edit: Cell<HWND>,
    font: Cell<HFONT>,
    font_dpi: Cell<u32>,
    previous_foreground: Cell<HWND>,
    visible: Cell<bool>,
    submission: RefCell<Option<CaptureSubmission>>,
}

impl Shared {
    fn hide(&self, restore_focus: bool) {
        if !self.visible.replace(false) {
            return;
        }
        unsafe {
            let _ = ShowWindow(self.popup.get(), SW_HIDE);
            let previous = self.previous_foreground.replace(HWND::default());
            if restore_focus && !previous.is_invalid() && IsWindow(Some(previous)).as_bool() {
                let _ = SetForegroundWindow(previous);
            }
        }
    }

    fn submit(&self) {
        let edit = self.edit.get();
        let text = unsafe {
            let length = usize::try_from(GetWindowTextLengthW(edit)).unwrap_or(0);
            let mut buffer = vec![0u16; length + 1];
            let copied = usize::try_from(GetWindowTextW(edit, &mut buffer)).unwrap_or(0);
            String::from_utf16_lossy(&buffer[..copied.min(length)])
        };
        let keep_open = unsafe { GetKeyState(i32::from(VK_SHIFT.0)) } < 0;
        *self.submission.borrow_mut() = Some(CaptureSubmission { text, keep_open });
        if keep_open {
            unsafe {
                let _ = SetWindowTextW(edit, w!(""));
            }
        } else {
            self.hide(true);
        }
        unsafe {
            let _ = PostMessageW(Some(self.owner), self.notify_message, WPARAM(0), LPARAM(0));
        }
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
}

pub struct QuickCaptureInput {
    shared: Box<Shared>,
}

impl QuickCaptureInput {
    #[must_use]
    pub fn create(owner: isize, notify_message: u32) -> Option<Self> {
        let shared = Box::new(Shared {
            owner: HWND(owner as *mut c_void),
            notify_message,
            popup: Cell::new(HWND::default()),
            edit: Cell::new(HWND::default()),
            font: Cell::new(HFONT::default()),
            font_dpi: Cell::new(0),
            previous_foreground: Cell::new(HWND::default()),
            visible: Cell::new(false),
            submission: RefCell::new(None),
        });
        let instance = unsafe { GetModuleHandleW(None) }.ok()?.into();
        unsafe {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(popup_proc),
                hInstance: instance,
                hCursor: LoadCursorW(None, IDC_IBEAM).unwrap_or_default(),
                hbrBackground: black_brush(),
                lpszClassName: POPUP_CLASS,
                ..Default::default()
            };
            RegisterClassExW(&class);
            let shared_ptr = std::ptr::from_ref::<Shared>(&shared).cast::<c_void>();
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
                Some(shared_ptr),
            )
            .ok()?;
            shared.popup.set(popup);
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
            );
            let Ok(edit) = edit else {
                let _ = DestroyWindow(popup);
                return None;
            };
            shared.edit.set(edit);
            SendMessageW(
                edit,
                EM_SETLIMITTEXT,
                Some(WPARAM(MAX_TASK_TEXT_BYTES)),
                Some(LPARAM(0)),
            );
            let installed = SetWindowSubclass(
                edit,
                Some(edit_proc),
                EDIT_SUBCLASS_ID,
                std::ptr::from_ref::<Shared>(&shared) as usize,
            );
            if !installed.as_bool() {
                let _ = DestroyWindow(popup);
                return None;
            }
        }
        Some(Self { shared })
    }

    pub fn show(&self, notch_center_x: i32, notch_bottom: i32, dpi: u32) {
        let shared = &*self.shared;
        let width = scale(POPUP_WIDTH_POINTS, dpi);
        let height = scale(POPUP_HEIGHT_POINTS, dpi);
        let pad = scale(POPUP_PAD_POINTS, dpi);
        let edit_height = scale(EDIT_HEIGHT_POINTS, dpi);
        shared.apply_font(dpi);
        unsafe {
            let region = CreateRoundRectRgn(
                0,
                0,
                width + 1,
                height + 1,
                scale(POPUP_RADIUS_POINTS, dpi) * 2,
                scale(POPUP_RADIUS_POINTS, dpi) * 2,
            );
            let _ = SetWindowRgn(shared.popup.get(), Some(region), true);
            let _ = SetWindowPos(
                shared.edit.get(),
                None,
                pad,
                (height - edit_height) / 2,
                width - pad * 2,
                edit_height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = SetWindowTextW(shared.edit.get(), w!(""));
            if !shared.visible.get() {
                shared.previous_foreground.set(GetForegroundWindow());
            }
            shared.visible.set(true);
            let _ = SetWindowPos(
                shared.popup.get(),
                Some(HWND_TOPMOST),
                notch_center_x - width / 2,
                notch_bottom + scale(POPUP_GAP_POINTS, dpi),
                width,
                height,
                SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(shared.popup.get());
            let _ = SetFocus(Some(shared.edit.get()));
        }
    }

    pub fn hide(&self) {
        self.shared.hide(true);
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.shared.visible.get()
    }

    #[must_use]
    pub fn take_submission(&self) -> Option<CaptureSubmission> {
        self.shared.submission.borrow_mut().take()
    }
}

impl Drop for QuickCaptureInput {
    fn drop(&mut self) {
        let shared = &*self.shared;
        shared.visible.set(false);
        unsafe {
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
        }
    }
}

const WINDOW_STYLE_EDIT: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE =
    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_LEFT as u32);

fn black_brush() -> HBRUSH {
    HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0)
}

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
            unsafe {
                SetTextColor(hdc, TEXT_COLOR);
                SetBkColor(hdc, BACKGROUND_COLOR);
            }
            return LRESULT(black_brush().0 as isize);
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
                shared.hide(true);
                return LRESULT(0);
            }
            WM_CHAR if wparam.0 == 13 || wparam.0 == 27 => return LRESULT(0),
            WM_KILLFOCUS => shared.hide(false),
            _ => {}
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::{CaptureSubmission, QuickCaptureInput};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_RETURN};
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_KEYDOWN, WM_SETTEXT};
    use windows::core::HSTRING;

    fn type_text(input: &QuickCaptureInput, text: &str) {
        let wide = HSTRING::from(text);
        unsafe {
            SendMessageW(
                input.shared.edit.get(),
                WM_SETTEXT,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
        }
    }

    fn press(input: &QuickCaptureInput, key: u16) {
        unsafe {
            SendMessageW(
                input.shared.edit.get(),
                WM_KEYDOWN,
                Some(WPARAM(usize::from(key))),
                Some(LPARAM(0)),
            );
        }
    }

    #[test]
    fn enter_submits_the_typed_text_and_hides_the_popup() {
        let input = QuickCaptureInput::create(0, 0).expect("popup");
        input.show(500, 40, 96);
        assert!(input.is_visible());
        type_text(&input, "kirim invoice");
        press(&input, VK_RETURN.0);
        assert_eq!(
            input.take_submission(),
            Some(CaptureSubmission {
                text: String::from("kirim invoice"),
                keep_open: false,
            })
        );
        assert!(!input.is_visible());
        assert_eq!(input.take_submission(), None);
    }

    #[test]
    fn escape_cancels_without_a_submission() {
        let input = QuickCaptureInput::create(0, 0).expect("popup");
        input.show(500, 40, 96);
        type_text(&input, "tidak jadi");
        press(&input, VK_ESCAPE.0);
        assert!(!input.is_visible());
        assert_eq!(input.take_submission(), None);
    }
}
