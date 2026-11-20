use otto_core::MotionPreference;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SPI_GETUIEFFECTS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};
use windows::core::BOOL;

#[must_use]
pub fn motion_preference() -> MotionPreference {
    let client_area_animation = read_setting(SPI_GETCLIENTAREAANIMATION);
    let ui_effects = read_setting(SPI_GETUIEFFECTS);
    let enabled = client_area_animation.unwrap_or(true) && ui_effects.unwrap_or(true);
    MotionPreference::from_animations_enabled(enabled)
}

fn read_setting(
    action: windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_ACTION,
) -> Option<bool> {
    let mut value = BOOL(1);
    unsafe {
        SystemParametersInfoW(
            action,
            0,
            Some(std::ptr::from_mut(&mut value).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()
        .map(|()| value.as_bool())
    }
}
