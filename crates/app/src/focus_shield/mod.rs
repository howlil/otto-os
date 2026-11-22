use otto_core::{FocusShieldDismissal, Rect, focus_shield_layout, is_back_hit};
use otto_render::FocusShieldRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN;

use crate::host::RuntimeContext;
use crate::host::win_msg::WM_LBUTTONUP;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShieldTarget {
    pub(crate) window: isize,
    pub(crate) label: String,
    pub(crate) page: String,
    pub(crate) browser: bool,
}

#[derive(Clone)]
struct Shown {
    target: ShieldTarget,
    frame: Rect,
    dpi: u32,
}

#[derive(Default)]
pub(crate) struct FocusShield {
    window: Option<isize>,
    renderer: Option<FocusShieldRenderer>,
    renderer_size: (i32, i32),
    shown: Option<Shown>,
    dismissal: FocusShieldDismissal,
}

impl FocusShield {
    fn paint(&mut self, window: isize, frame: Rect, dpi: u32, label: &str) -> bool {
        let size = (frame.width, frame.height);
        if self.renderer.is_none() || self.renderer_size != size {
            match FocusShieldRenderer::new(size.0, size.1) {
                Ok(renderer) => {
                    self.renderer = Some(renderer);
                    self.renderer_size = size;
                }
                Err(error) => {
                    otto_platform::log_error("focus-shield", &format!("surface failed: {error}"));
                    return false;
                }
            }
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        match renderer.present(window, (frame.x, frame.y), dpi, label) {
            Ok(()) => true,
            Err(error) => {
                otto_platform::log_error("focus-shield", &format!("present failed: {error}"));
                false
            }
        }
    }
}

pub(crate) fn may_block(runtime: &RuntimeContext, window: isize, page: &str) -> bool {
    runtime
        .focus_shield
        .try_borrow_mut()
        .is_ok_and(|mut state| {
            let covered = state
                .shown
                .as_ref()
                .is_some_and(|shown| shown.target.window == window);
            !covered && !state.dismissal.suppresses(window, page)
        })
}

pub(crate) fn block(runtime: &RuntimeContext, target: ShieldTarget) {
    let Some(frame) = otto_platform::window_frame(target.window) else {
        return;
    };
    let dpi = otto_platform::window_dpi(target.window);
    let Some(window) = ensure_window(runtime) else {
        return;
    };
    let painted = runtime
        .focus_shield
        .try_borrow_mut()
        .is_ok_and(|mut state| {
            let painted = state.paint(window, frame, dpi, &target.label);
            if painted {
                state.shown = Some(Shown { target, frame, dpi });
            }
            painted
        });
    if painted {
        otto_platform::show_over(window, frame);
    }
}

pub(crate) fn sync(runtime: &RuntimeContext, focus_running: bool) {
    let Some(shown) = runtime
        .focus_shield
        .try_borrow()
        .ok()
        .and_then(|state| state.shown.clone())
    else {
        if !focus_running && let Ok(mut state) = runtime.focus_shield.try_borrow_mut() {
            state.dismissal = FocusShieldDismissal::default();
        }
        return;
    };
    let shield = runtime
        .focus_shield
        .try_borrow()
        .ok()
        .and_then(|state| state.window);
    let focused = otto_platform::foreground_window()
        .is_some_and(|foreground| foreground == shown.target.window || Some(foreground) == shield);
    match otto_platform::window_frame(shown.target.window) {
        Some(frame) if focus_running && focused => {
            if frame != shown.frame {
                reposition(runtime, shown, frame);
            }
        }
        _ => hide(runtime),
    }
}

fn reposition(runtime: &RuntimeContext, shown: Shown, frame: Rect) {
    let dpi = otto_platform::window_dpi(shown.target.window);
    let placed = runtime
        .focus_shield
        .try_borrow_mut()
        .ok()
        .and_then(|mut state| {
            let window = state.window?;
            if !state.paint(window, frame, dpi, &shown.target.label) {
                return None;
            }
            state.shown = Some(Shown {
                target: shown.target,
                frame,
                dpi,
            });
            Some(window)
        });
    if let Some(window) = placed {
        otto_platform::place(window, frame);
    }
}

fn hide(runtime: &RuntimeContext) {
    let window = runtime
        .focus_shield
        .try_borrow_mut()
        .ok()
        .and_then(|mut state| {
            state.shown = None;
            state.window
        });
    if let Some(window) = window {
        otto_platform::hide_window(window);
    }
}

pub(crate) fn back(runtime: &RuntimeContext) {
    let exit = runtime
        .focus_shield
        .try_borrow_mut()
        .ok()
        .and_then(|mut state| {
            let shown = state.shown.take()?;
            state
                .dismissal
                .dismiss(shown.target.window, &shown.target.page);
            Some((shown.target, state.window))
        });
    let Some((target, window)) = exit else {
        return;
    };
    if let Some(window) = window {
        otto_platform::hide_window(window);
    }
    if target.browser {
        otto_platform::back_in_browser(target.window);
    } else if let Some(below) = otto_platform::window_below(target.window) {
        otto_platform::activate_window(below);
    }
}

pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let back_requested = match message {
        WM_LBUTTONUP => {
            let (x, y) = point(lparam);
            runtime
                .focus_shield
                .try_borrow()
                .ok()
                .and_then(|state| state.shown.clone())
                .is_some_and(|shown| {
                    let layout =
                        focus_shield_layout(shown.frame.width, shown.frame.height, shown.dpi);
                    is_back_hit(&layout, x, y)
                })
        }
        WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => true,
        _ => return None,
    };
    if back_requested {
        back(runtime);
    }
    Some(LRESULT(0))
}

pub(crate) fn shutdown(runtime: &RuntimeContext) -> Vec<HWND> {
    runtime
        .focus_shield
        .try_borrow_mut()
        .ok()
        .and_then(|mut state| {
            state.shown = None;
            state.renderer = None;
            state.window.take()
        })
        .map(|window| vec![HWND(window as *mut _)])
        .unwrap_or_default()
}

fn ensure_window(runtime: &RuntimeContext) -> Option<isize> {
    let existing = runtime
        .focus_shield
        .try_borrow()
        .ok()
        .and_then(|state| state.window);
    if existing.is_some() {
        return existing;
    }
    let window = crate::host::create_focus_shield_window()?;
    runtime.focus_shield.try_borrow_mut().ok()?.window = Some(window);
    Some(window)
}

fn point(lparam: LPARAM) -> (i32, i32) {
    let bits = lparam.0 as u32;
    (
        i32::from(bits as u16 as i16),
        i32::from((bits >> 16) as u16 as i16),
    )
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::LPARAM;

    use super::point;

    #[test]
    fn point_reads_signed_client_coordinates_from_lparam() {
        let packed = LPARAM(isize::try_from(0xFFFD_u32 | (5_u32 << 16)).unwrap_or_default());
        assert_eq!(point(packed), (-3, 5));
    }
}
