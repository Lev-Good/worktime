//! Detects a machine that went to sleep or a user who stepped away, so a running timer
//! does not charge time nobody worked.
//!
//! Windows reports how long the keyboard and mouse have been quiet, which is the only
//! reliable "is anyone there" signal a tray app has. Other platforms return `None`, and the
//! ticker then relies on the clock alone, which still catches a suspended machine.

/// How long the one-second ticker may miss its beat before the gap counts as a suspended
/// machine rather than a busy moment.
pub const SLEEP_GAP_SECONDS: i64 = 90;

/// Milliseconds since the last keyboard or mouse input, when the platform can tell.
#[cfg(windows)]
pub fn idle_millis() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid LASTINPUTINFO with `cbSize` set, as the API requires, and
    // the pointer is only used for the duration of the call. Both values are milliseconds
    // from the same tick counter, so the subtraction tolerates its 32-bit wraparound.
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return None;
        }
        Some(GetTickCount().wrapping_sub(info.dwTime) as u64)
    }
}

/// No keyboard or mouse signal is available here, so the ticker falls back to clock gaps.
#[cfg(not(windows))]
pub fn idle_millis() -> Option<u64> {
    None
}
