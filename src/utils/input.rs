#![allow(dead_code)]

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticInputSource {
    Triggerbot,
    CameraTriggerbot,
    ViewportTriggerbot,
    SilentTriggerbot,
    BladeBall,
    AutoClicker,
    Unknown,
}

impl SyntheticInputSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Triggerbot => "triggerbot",
            Self::CameraTriggerbot => "camera_triggerbot",
            Self::ViewportTriggerbot => "viewport_triggerbot",
            Self::SilentTriggerbot => "silent_triggerbot",
            Self::BladeBall => "blade_ball",
            Self::AutoClicker => "autoclicker",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SyntheticClickStats {
    pub total: u64,
    pub triggerbot: u64,
    pub camera_triggerbot: u64,
    pub viewport_triggerbot: u64,
    pub silent_triggerbot: u64,
    pub blade_ball: u64,
    pub autoclicker: u64,
    pub unknown: u64,
}

static SYN_TOTAL: AtomicU64 = AtomicU64::new(0);
static SYN_TRIGGERBOT: AtomicU64 = AtomicU64::new(0);
static SYN_CAMERA_TRIGGERBOT: AtomicU64 = AtomicU64::new(0);
static SYN_VIEWPORT_TRIGGERBOT: AtomicU64 = AtomicU64::new(0);
static SYN_SILENT_TRIGGERBOT: AtomicU64 = AtomicU64::new(0);
static SYN_BLADE_BALL: AtomicU64 = AtomicU64::new(0);
static SYN_AUTOCLICKER: AtomicU64 = AtomicU64::new(0);
static SYN_UNKNOWN: AtomicU64 = AtomicU64::new(0);

#[inline]
fn bump_source_counter(source: SyntheticInputSource) {
    match source {
        SyntheticInputSource::Triggerbot => { SYN_TRIGGERBOT.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::CameraTriggerbot => { SYN_CAMERA_TRIGGERBOT.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::ViewportTriggerbot => { SYN_VIEWPORT_TRIGGERBOT.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::SilentTriggerbot => { SYN_SILENT_TRIGGERBOT.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::BladeBall => { SYN_BLADE_BALL.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::AutoClicker => { SYN_AUTOCLICKER.fetch_add(1, Ordering::Relaxed); }
        SyntheticInputSource::Unknown => { SYN_UNKNOWN.fetch_add(1, Ordering::Relaxed); }
    }
}

pub struct Input;

impl Input {
    #[cfg(target_os = "windows")]
    pub fn is_key_down(vk_code: i32) -> bool {
        unsafe { GetAsyncKeyState(vk_code) < 0 }
    }

    pub fn is_key_pressed(vk_code: i32, state: &mut bool) -> bool {
        let pressed = Self::is_key_down(vk_code);
        let just_pressed = pressed && !*state;
        *state = pressed;
        just_pressed
    }

    #[cfg(target_os = "windows")]
    pub fn move_mouse(dx: i32, dy: i32) {
        unsafe {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }


    pub fn note_synthetic_click(source: SyntheticInputSource, _button: &'static str) {
        SYN_TOTAL.fetch_add(1, Ordering::Relaxed);
        bump_source_counter(source);
    }

    pub fn take_synthetic_click_stats() -> SyntheticClickStats {
        SyntheticClickStats {
            total: SYN_TOTAL.swap(0, Ordering::Relaxed),
            triggerbot: SYN_TRIGGERBOT.swap(0, Ordering::Relaxed),
            camera_triggerbot: SYN_CAMERA_TRIGGERBOT.swap(0, Ordering::Relaxed),
            viewport_triggerbot: SYN_VIEWPORT_TRIGGERBOT.swap(0, Ordering::Relaxed),
            silent_triggerbot: SYN_SILENT_TRIGGERBOT.swap(0, Ordering::Relaxed),
            blade_ball: SYN_BLADE_BALL.swap(0, Ordering::Relaxed),
            autoclicker: SYN_AUTOCLICKER.swap(0, Ordering::Relaxed),
            unknown: SYN_UNKNOWN.swap(0, Ordering::Relaxed),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn click_mouse() {
        Self::click_mouse_from(SyntheticInputSource::Unknown);
    }

    #[cfg(target_os = "windows")]
    pub fn click_mouse_from(source: SyntheticInputSource) {
        Self::note_synthetic_click(source, "LMB");
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(40));
            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn click_mouse_fast_from(source: SyntheticInputSource) {
        Self::note_synthetic_click(source, "LMB");
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(12));
            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }


    #[cfg(target_os = "windows")]
    pub fn mouse_down_from(source: SyntheticInputSource) {
        Self::note_synthetic_click(source, "LMB");
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn mouse_up_from(_source: SyntheticInputSource) {
        unsafe {
            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn mouse_down() {
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn mouse_up() {
        unsafe {
            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn get_movement_keys() -> (bool, bool, bool, bool, bool, bool) {
        (
            Self::is_key_down(0x57), // W
            Self::is_key_down(0x41), // A
            Self::is_key_down(0x53), // S
            Self::is_key_down(0x44), // D
            Self::is_key_down(0x20), // Space (up)
            Self::is_key_down(0x11) || Self::is_key_down(0xA2) || Self::is_key_down(0xA3), // Ctrl/LCtrl/RCtrl (down)
        )
    }

    #[cfg(target_os = "windows")]
    pub fn get_mouse_position() -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        use windows::Win32::Foundation::POINT;
        
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut point);
        }
        (point.x, point.y)
    }

    #[cfg(target_os = "windows")]
    pub fn set_mouse_position(x: i32, y: i32) -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
        use windows::Win32::Foundation::POINT;
        
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut point);
            let _ = SetCursorPos(x, y);
        }
        (point.x, point.y)
    }

    #[cfg(target_os = "windows")]
    pub fn get_screen_center() -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (width / 2, height / 2)
        }
    }

}

#[allow(dead_code)]
pub mod vk {
    pub const LBUTTON: i32 = 0x01;
    pub const RBUTTON: i32 = 0x02;
    pub const SPACE: i32 = 0x20;
    pub const SHIFT: i32 = 0x10;
    pub const CTRL: i32 = 0x11;
    pub const ALT: i32 = 0x12;
    
    pub const KEY_W: i32 = 0x57;
    pub const KEY_A: i32 = 0x41;
    pub const KEY_S: i32 = 0x53;
    pub const KEY_D: i32 = 0x44;
    pub const KEY_G: i32 = 0x47;
    
    pub const F1: i32 = 0x70;
    pub const F2: i32 = 0x71;
    pub const F3: i32 = 0x72;
    pub const F4: i32 = 0x73;
    pub const F5: i32 = 0x74;
    pub const F6: i32 = 0x75;
    pub const F7: i32 = 0x76;
    pub const F8: i32 = 0x77;
    pub const F9: i32 = 0x78;
    pub const F10: i32 = 0x79;
    pub const F11: i32 = 0x7A;
    pub const F12: i32 = 0x7B;
}
