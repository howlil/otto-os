pub(crate) const WM_MOUSELEAVE: u32 = 0x02A3;
pub(crate) const WM_GETOBJECT: u32 = 0x003D;

const WM_APP_BASE: u32 = 0x8000;

pub(crate) const WM_APP_WINDOW_SYNC: u32 = WM_APP_BASE + 20;
pub(crate) const WM_APP_BAR_KEY: u32 = WM_APP_BASE + 21;
pub(crate) const WM_APP_IDLE_WAKE: u32 = WM_APP_BASE + 22;
pub(crate) const WM_APP_DOCK_KEY: u32 = WM_APP_BASE + 23;
pub(crate) const WM_APP_BAR_BACKDROP: u32 = WM_APP_BASE + 24;
pub(crate) const WM_APP_NATIVE_MINIMIZE: u32 = WM_APP_BASE + 24;
pub(crate) const WM_APP_DOCK_BACKDROP: u32 = WM_APP_BASE + 25;
pub(crate) const WM_APP_GENIE_DONE: u32 = WM_APP_BASE + 27;
pub(crate) const WM_APP_BAR_BLUETOOTH_HUD: u32 = WM_APP_BASE + 28;
pub(crate) const WM_APP_SPOTLIGHT_BACKDROP: u32 = WM_APP_BASE + 29;
pub(crate) const WM_APP_BAR_CAPTURE: u32 = WM_APP_BASE + 60;
pub(crate) const WM_APP_CONTROL_CENTER_KEY: u32 = WM_APP_BASE + 26;
pub(crate) const WM_APP_CONTROL_CENTER_BACKDROP: u32 = WM_APP_BASE + 27;
pub(crate) const WM_APP_MENU_BACKDROP: u32 = WM_APP_BASE + 28;

pub(crate) const STATUS_TIMER: usize = 1;
pub(crate) const HUD_ANIM_TIMER: usize = 2;
pub(crate) const DEADLINE_TIMER: usize = 3;
pub(crate) const DOCK_POLL_TIMER: usize = 10;
pub(crate) const DOCK_ANIM_TIMER: usize = 11;
pub(crate) const WINDOW_CONTROLS_ANIM_TIMER: usize = 20;
pub(crate) const GREEN_HOVER_TIMER: usize = 21;
pub(crate) const SPOTLIGHT_TIMER: usize = 32;
pub(crate) const CC_POLL_TIMER: usize = 31;
pub(crate) const MENU_POLL_TIMER: usize = 30;
pub(crate) const UPDATE_TIMER_ID: usize = 0x5E7;

pub(crate) const WM_POINTERUPDATE: u32 = 0x0245;
pub(crate) const WM_POINTERDOWN: u32 = 0x0246;
pub(crate) const WM_POINTERUP: u32 = 0x0247;
pub(crate) const WM_POINTERCAPTURECHANGED: u32 = 0x024C;
pub(crate) const WM_POWERBROADCAST: u32 = 0x0218;
pub(crate) const WM_DEVICECHANGE: u32 = 0x0219;
pub(crate) const WM_LBUTTONUP: u32 = 0x0202;
pub(crate) const WM_DROPFILES: u32 = 0x0233;
pub(crate) const WM_UNICHAR: u32 = 0x0109;
pub(crate) const DBT_DEVNODES_CHANGED: usize = 0x0007;
pub(crate) const DBT_DEVICEARRIVAL: usize = 0x8000;
pub(crate) const DBT_DEVICEREMOVECOMPLETE: usize = 0x8004;
pub(crate) const PBT_APMSUSPEND: usize = 0x0004;
pub(crate) const PBT_APMRESUMESUSPEND: usize = 0x0007;
pub(crate) const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;

#[cfg(test)]
mod tests {
    use super::*;

    fn all_distinct<T: Ord + Copy>(values: &[T]) -> bool {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        sorted.windows(2).all(|pair| pair[0] != pair[1])
    }

    #[test]
    fn messages_are_distinct_within_each_target_window() {
        assert!(all_distinct(&[
            WM_APP_BAR_KEY,
            WM_APP_IDLE_WAKE,
            WM_APP_BAR_BACKDROP,
            WM_APP_BAR_BLUETOOTH_HUD,
            WM_APP_BAR_CAPTURE,
        ]));
        assert!(all_distinct(&[WM_APP_DOCK_KEY, WM_APP_DOCK_BACKDROP]));
        assert!(all_distinct(&[WM_APP_WINDOW_SYNC, WM_APP_NATIVE_MINIMIZE]));
    }

    #[test]
    fn timers_are_distinct_within_each_target_window() {
        assert!(all_distinct(&[
            STATUS_TIMER,
            HUD_ANIM_TIMER,
            DEADLINE_TIMER
        ]));
        assert!(all_distinct(&[DOCK_POLL_TIMER, DOCK_ANIM_TIMER]));
        assert!(all_distinct(&[
            WINDOW_CONTROLS_ANIM_TIMER,
            GREEN_HOVER_TIMER
        ]));
    }
}
