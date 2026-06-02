//Configuration management module.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;

// Error Types

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
    
    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
}

// Configuration Structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub visuals: VisualsConfig,
    pub aimbot: AimbotConfig,
    #[serde(default)]
    pub camera_aim: CameraAimConfig,
    #[serde(default)]
    pub mouse_aim: MouseAimConfig,
    #[serde(default)]
    pub silent_aim: SilentAimConfig,
    #[serde(default)]
    pub viewport_aim: ViewportAimConfig,
    #[serde(default)]
    pub movement: MovementConfig,
    pub world: WorldConfig,
    pub camera: CameraConfig,
    pub interface: InterfaceConfig,
    pub performance: PerformanceConfig,
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub hotkey_bindings: HotkeyBindings,
    #[serde(default)]
    pub autoclicker: AutoClickerConfig,
    #[serde(default)]
    pub hitbox: HitboxConfig,
    #[serde(default)]
    pub anti_afk: AntiAfkConfig
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub username: String,
    pub process_name: String,
    pub window_title: String,
    pub startup_delay_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualsConfig {
    pub box_esp: bool,
    pub name_tags: bool,
    pub health_bars: bool,
    pub armor_bars: bool,
    pub tracers: bool,
    pub chams: bool,
    pub distance_colors: bool,
    pub target_highlight: bool,
    pub max_distance: f32,
    #[serde(default)]
    pub team_check: bool,
    #[serde(default)]
    pub hide_dead: bool,
    #[serde(default)]
    pub hide_transparent: bool,
    #[serde(default)]
    pub zombies_mode: bool,
    #[serde(default)]
    pub show_bots: bool,
    #[serde(default)]
    pub teammate_whitelist: Vec<String>,
    // Box styling
    #[serde(default)]
    pub box_style: u8,
    #[serde(default)]
    pub box_fill: bool,
    #[serde(default = "default_box_color")]
    pub box_color: [f32; 4],
    #[serde(default = "default_box_fill_color")]
    pub box_fill_color: [f32; 4],
    #[serde(default = "default_box_fill_opacity")]
    pub box_fill_opacity: f32,
    // ESP label colors/sizes/positions
    #[serde(default = "default_esp_name_color")]
    pub esp_name_color: [f32; 4],
    #[serde(default = "default_esp_font_size")]
    pub esp_name_size: f32,
    #[serde(default)]
    pub esp_name_pos: u8,
    #[serde(default = "default_esp_dist_color")]
    pub esp_dist_color: [f32; 4],
    #[serde(default = "default_esp_font_size")]
    pub esp_dist_size: f32,
    #[serde(default)]
    pub esp_dist_pos: u8,
    #[serde(default = "default_esp_weapon_color")]
    pub esp_weapon_color: [f32; 4],
    #[serde(default = "default_esp_font_size")]
    pub esp_weapon_size: f32,
    #[serde(default)]
    pub esp_weapon_pos: u8,
    #[serde(default)]
    pub show_distance_label: bool,
    #[serde(default)]
    pub show_equipped_weapon: bool,
    #[serde(default)]
    pub team_hide_visuals: bool,
    #[serde(default)]
    pub wall_check: bool,
    #[serde(default)]
    pub esp_gpu_rendering: bool,
}

fn default_box_color() -> [f32; 4] { [1.0, 0.2, 0.2, 0.9] }
fn default_box_fill_color() -> [f32; 4] { [1.0, 0.2, 0.2, 0.15] }
fn default_box_fill_opacity() -> f32 { 0.15 }
fn default_esp_name_color() -> [f32; 4] { [1.0, 1.0, 1.0, 1.0] }
fn default_esp_dist_color() -> [f32; 4] { [0.8, 0.8, 0.8, 0.9] }
fn default_esp_weapon_color() -> [f32; 4] { [0.9, 0.8, 0.4, 1.0] }
fn default_esp_font_size() -> f32 { 12.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimbotConfig {
    pub enabled: bool,
    pub fov: f32,
    pub smoothing: f32,
    pub show_fov: bool,
    pub target_bone: String,
    pub prediction_enabled: bool,
    pub prediction_amount: f32,
    #[serde(default)]
    pub mode: u8,
    #[serde(default)]
    pub activation_mode: u8,
    #[serde(default = "default_hold_delay")]
    pub hold_delay_ms: u32,
    #[serde(default)]
    pub auto_reload: bool,
    #[serde(default)]
    pub prioritize_health: bool,
    #[serde(default)]
    pub sens_compensation: bool,
    #[serde(default)]
    pub ground_offset_enabled: bool,
    #[serde(default = "default_ground_offset_y")]
    pub ground_offset_y: f32,
}

fn default_hold_delay() -> u32 {50}
fn default_ground_offset_y() -> f32 { 1.5 }

// /// Triggerbot- Feature is DISABLED for demo. 
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct TriggerbotConfig {
//     pub enabled: bool,
//     pub delay_ms: f32,
//     #[serde(default = "default_trigger_radius")]
//     pub trigger_radius: f32,
// }

// fn default_trigger_radius() -> f32 { 8.0 }

/// Camera Aim - CFrame rotation spoofing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CameraAimConfig {
    pub enabled: bool, 
    #[serde(default = "default_camera_aim_fov")]
    pub fov: f32, // Show FOV circle
    #[serde(default)]
    pub show_fov: bool, 
    #[serde(default = "default_target_bone")]
    pub target_bone: String,
}

fn default_camera_aim_fov() -> f32 { 150.0 }
fn default_target_bone() -> String { "Head".to_string() }

/// Mouse-move (SendInput) aim — full implementation in features/aimbot/mouse_aim.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MouseAimConfig {
    pub enabled: bool,
    #[serde(default = "default_mouse_aim_fov")]
    pub fov: f32,
    #[serde(default)]
    pub show_fov: bool,
    #[serde(default = "default_target_bone")]
    pub target_bone: String,
    #[serde(default = "default_mouse_smoothing")]
    pub smoothing: f32,
}

fn default_mouse_aim_fov() -> f32 { 150.0 }
fn default_mouse_smoothing() -> f32 { 5.0 }

/// Silent aim (MouseService write) — stub for demo
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilentAimConfig {
    pub enabled: bool,
    #[serde(default = "default_mouse_aim_fov")]
    pub fov: f32,
    #[serde(default)]
    pub show_fov: bool,
}

/// Viewport aim (CFrame camera write) — stub for demo
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViewportAimConfig {
    pub enabled: bool,
    #[serde(default = "default_mouse_aim_fov")]
    pub fov: f32,
    #[serde(default)]
    pub show_fov: bool,
    #[serde(default = "default_target_bone")]
    pub target_bone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementConfig {
    pub jump_power: f32,
    pub walk_speed: f32,
    pub fly_enabled: bool,
    pub fly_speed: f32,
    pub noclip_enabled: bool,
    pub auto_jump: bool,
    #[serde(default = "default_write_intensity")]
    pub write_intensity: u8, 
    #[serde(default)]
    pub fly_mode: u8,
    #[serde(default)]
    pub spinbot_enabled: bool,
    #[serde(default = "default_spinbot_speed")]
    pub spinbot_speed: f32,
    #[serde(default)]
    pub anti_sit_enabled: bool,
    #[serde(default)]
    pub hip_height_enabled: bool,
    #[serde(default = "default_hip_height")]
    pub hip_height_value: f32,     // Hip Height value (default is 2.0)
    #[serde(default)]
    pub void_hide_enabled: bool,
}

fn default_spinbot_speed() -> f32 { 6.0 } 
fn default_hip_height() -> f32 { 2.0 }
fn default_write_intensity() -> u8 { 3 } 

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    pub anti_fog: bool,
    pub fog_start: f32,
    pub fog_end: f32,
    pub brightness_enabled: bool,
    pub brightness_value: f32,
    pub anti_flash: bool,
    pub max_brightness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub fov_enabled: bool,
    pub fov_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub show_hotkey_hints: bool,
    #[serde(default = "default_hotkey_x")]
    pub hotkey_pos_x: f32,
    #[serde(default = "default_hotkey_y")]
    pub hotkey_pos_y: f32,
    #[serde(default = "default_hotkey_anchor")]
    pub hotkey_anchor: String, 
    #[serde(default)]
    pub expanded_aim_section: u8,
    #[serde(default = "default_accent_r")]
    pub accent_r: u8,
    #[serde(default = "default_accent_g")]
    pub accent_g: u8,
    #[serde(default = "default_accent_b")]
    pub accent_b: u8,
}

fn default_hotkey_x() -> f32 { 10.0 }
fn default_hotkey_y() -> f32 { 10.0 }
fn default_hotkey_anchor() -> String { "top-left".to_string() }
fn default_accent_r() -> u8 { 100 }
fn default_accent_g() -> u8 { 100 }
fn default_accent_b() -> u8 { 220 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub target_fps: u32,
    pub cache_update_ms: u64,
    pub idle_mode: bool,
    pub idle_fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntiAfkConfig {
    pub enabled: bool,
    #[serde(default = "default_anti_afk_interval")]
    pub interval_secs: u32,
}

fn default_anti_afk_interval() -> u32 {
    60 // Default 60 seconds
}


/// Feature toggles are now handled by HotkeyBindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    // F1 - Toggle menu
    pub menu_toggle: u32,
    // F9 - Reload player cache
    pub reload_data: u32,
    // F12 - Exit application
    pub exit: u32,
    // RMB - Aim activation key
    pub aim_key: u32,
    // Insert - Autoclicker toggle
    #[serde(default = "default_autoclicker_toggle")]
    pub autoclicker_toggle: u32,
    // Home - Full game instance refresh
    #[serde(default = "default_full_refresh")]
    pub full_refresh: u32,
    // End - Save config to file
    #[serde(default = "default_save_config")]
    pub save_config: u32,
}

fn default_autoclicker_toggle() -> u32 {
    0x2D // Insert
}

fn default_full_refresh() -> u32 {
    0x24 // Home
}

fn default_save_config() -> u32 {
    0x23 // End
}

/// Features that can be bound to hotkeys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BindableFeature {
    #[default]
    None,
    // Visuals
    BoxEsp,
    NameTags,
    Tracers,
    HealthBars,
    ArmourBars,
    Chams,
    TeamCheck,
    HideDead,
    ShowBots,
    // Aim
    AimAssist,
    Triggerbot,
    CameraAim,
    AutoReload,
    // Movement
    Fly,
    Noclip,
    Spinbot,
    AntiSit,
    VoidHide,
    // Hitbox
    HitboxMod,
    ShowHitboxVisual,
}

impl BindableFeature {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "-- None --",
            Self::BoxEsp => "Box ESP",
            Self::NameTags => "Name Tags",
            Self::Tracers => "Tracers",
            Self::HealthBars => "Health Bars",
            Self::ArmourBars => "Armour Bars",
            Self::Chams => "Chams",
            Self::TeamCheck => "Team Check",
            Self::HideDead => "Hide Dead",
            Self::ShowBots => "Show Bots",
            Self::AimAssist => "Aim Assist",
            Self::Triggerbot => "Triggerbot",
            Self::CameraAim => "Camera Aim",
            Self::AutoReload => "Auto Reload",
            Self::Fly => "Fly",
            Self::Noclip => "Noclip",
            Self::Spinbot => "Spinbot",
            Self::AntiSit => "Anti-Sit",
            Self::VoidHide => "Void Hide",
            Self::HitboxMod => "Hitbox Mod",
            Self::ShowHitboxVisual => "Show Hitbox Visual",
        }
    }
    pub fn category(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::BoxEsp | Self::NameTags | Self::Tracers | Self::HealthBars | 
            Self::ArmourBars | Self::Chams | Self::TeamCheck | Self::HideDead | Self::ShowBots => "VISUAL",
            Self::AimAssist | Self::Triggerbot | Self::CameraAim | Self::AutoReload => "AIM",
            Self::Fly | Self::Noclip | Self::Spinbot | Self::AntiSit | Self::VoidHide => "MOVEMENT",
            Self::HitboxMod | Self::ShowHitboxVisual => "HITBOX",
        }
    }
    pub fn all_features() -> &'static [BindableFeature] {
        &[
            Self::None,
            // Visuals
            Self::BoxEsp,
            Self::NameTags,
            Self::Tracers,
            Self::HealthBars,
            Self::ArmourBars,
            Self::Chams,
            Self::TeamCheck,
            Self::HideDead,
            Self::ShowBots,
            // Aim
            Self::AimAssist,
            Self::Triggerbot,
            Self::CameraAim,
            Self::AutoReload,
            // Movement
            Self::Fly,
            Self::Noclip,
            Self::Spinbot,
            Self::AntiSit,
            Self::VoidHide,
            // Hitbox
            Self::HitboxMod,
            Self::ShowHitboxVisual,
        ]
    }
}

/// Virtual key codes for hotkey binding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HotkeyKey {
    #[default]
    None,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Insert, Delete, Home, End, PageUp, PageDown,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, 
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
}

impl HotkeyKey {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::F1 => "F1", Self::F2 => "F2", Self::F3 => "F3", Self::F4 => "F4",
            Self::F5 => "F5", Self::F6 => "F6", Self::F7 => "F7", Self::F8 => "F8",
            Self::F9 => "F9", Self::F10 => "F10", Self::F11 => "F11", Self::F12 => "F12",
            Self::Insert => "Ins", Self::Delete => "Del", 
            Self::Home => "Home", Self::End => "End",
            Self::PageUp => "PgUp", Self::PageDown => "PgDn",
            Self::Numpad0 => "Num0", Self::Numpad1 => "Num1", Self::Numpad2 => "Num2",
            Self::Numpad3 => "Num3", Self::Numpad4 => "Num4", Self::Numpad5 => "Num5",
            Self::Numpad6 => "Num6", Self::Numpad7 => "Num7", Self::Numpad8 => "Num8",
            Self::Numpad9 => "Num9",
        }
    }
    
    pub fn to_vk_code(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::F1 => 0x70, Self::F2 => 0x71, Self::F3 => 0x72, Self::F4 => 0x73,
            Self::F5 => 0x74, Self::F6 => 0x75, Self::F7 => 0x76, Self::F8 => 0x77,
            Self::F9 => 0x78, Self::F10 => 0x79, Self::F11 => 0x7A, Self::F12 => 0x7B,
            Self::Insert => 0x2D, Self::Delete => 0x2E,
            Self::Home => 0x24, Self::End => 0x23,
            Self::PageUp => 0x21, Self::PageDown => 0x22,
            Self::Numpad0 => 0x60, Self::Numpad1 => 0x61, Self::Numpad2 => 0x62,
            Self::Numpad3 => 0x63, Self::Numpad4 => 0x64, Self::Numpad5 => 0x65,
            Self::Numpad6 => 0x66, Self::Numpad7 => 0x67, Self::Numpad8 => 0x68,
            Self::Numpad9 => 0x69,
        }
    }
    
    pub fn all_keys() -> &'static [HotkeyKey] {
        &[
            Self::None,
            Self::F1, Self::F2, Self::F3, Self::F4, Self::F5, Self::F6,
            Self::F7, Self::F8, Self::F9, Self::F10, Self::F11, Self::F12,
            Self::Insert, Self::Delete, Self::Home, Self::End, Self::PageUp, Self::PageDown,
            Self::Numpad0, Self::Numpad1, Self::Numpad2, Self::Numpad3, Self::Numpad4,
            Self::Numpad5, Self::Numpad6, Self::Numpad7, Self::Numpad8, Self::Numpad9,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySlot {
    pub key: HotkeyKey,
    pub feature: BindableFeature,
}

impl Default for HotkeySlot {
    fn default() -> Self {
        Self {
            key: HotkeyKey::None,
            feature: BindableFeature::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBindings {
    pub slots: [HotkeySlot; 10],
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            slots: [
                HotkeySlot { key: HotkeyKey::F2, feature: BindableFeature::BoxEsp },
                HotkeySlot { key: HotkeyKey::F3, feature: BindableFeature::Chams },
                HotkeySlot { key: HotkeyKey::F4, feature: BindableFeature::AimAssist },
                // DISABLED: Triggerbot feature not production ready - slot available for reassignment
                HotkeySlot { key: HotkeyKey::F5, feature: BindableFeature::CameraAim },
                HotkeySlot { key: HotkeyKey::F6, feature: BindableFeature::Fly },
                HotkeySlot { key: HotkeyKey::F7, feature: BindableFeature::Tracers },
                HotkeySlot { key: HotkeyKey::F8, feature: BindableFeature::Noclip },
                HotkeySlot { key: HotkeyKey::F9, feature: BindableFeature::HitboxMod },
                HotkeySlot { key: HotkeyKey::Insert, feature: BindableFeature::Spinbot },
                HotkeySlot { key: HotkeyKey::None, feature: BindableFeature::None },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoClickerConfig {
    pub enabled: bool,
    pub delay_ms: f32,
    pub variance_percent: f32,
    pub hold_duration_ms: f32,
    #[serde(default)]
    pub turbo_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitboxConfig {
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub enemy_enabled: bool,
    pub head_scale: f32,
    pub torso_scale: f32,
    pub arms_scale: f32,
    pub legs_scale: f32,
    #[serde(default)]
    pub self_enabled: bool,
    #[serde(default = "default_self_scale")]
    pub self_scale: f32,
    #[serde(default = "default_hitbox_show_visual")]
    pub show_visual: bool,
    #[serde(default = "default_hitbox_color")]
    pub color: [f32; 4],
}

fn default_true() -> bool {
    true
}

fn default_self_scale() -> f32 {
    1.0
}

fn default_hitbox_show_visual() -> bool {
    true
}

fn default_hitbox_color() -> [f32; 4] {
    [0.0, 1.0, 0.5, 0.4] 
}


// Default Implementations
impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            visuals: VisualsConfig::default(),
            aimbot: AimbotConfig::default(),
            camera_aim: CameraAimConfig::default(),
            mouse_aim: MouseAimConfig::default(),
            silent_aim: SilentAimConfig::default(),
            viewport_aim: ViewportAimConfig::default(),
            movement: MovementConfig::default(),
            world: WorldConfig::default(),
            camera: CameraConfig::default(),
            interface: InterfaceConfig::default(),
            performance: PerformanceConfig::default(),
            hotkeys: HotkeyConfig::default(),
            hotkey_bindings: HotkeyBindings::default(),
            autoclicker: AutoClickerConfig::default(),
            hitbox: HitboxConfig::default(),
            anti_afk: AntiAfkConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            username: "YourUsernameHere".to_string(),
            process_name: "RobloxPlayerBeta.exe".to_string(),
            window_title: "Rust Project".to_string(),
            startup_delay_secs: 5,
        }
    }
}

impl Default for VisualsConfig {
    fn default() -> Self {
        Self {
            box_esp: false,
            name_tags: false,
            health_bars: false,
            armor_bars: false,
            tracers: false,
            chams: false,
            distance_colors: true,
            target_highlight: true,
            max_distance: 500.0,
            team_check: false,
            hide_dead: true,
            hide_transparent: false,
            zombies_mode: false,
            show_bots: false,
            teammate_whitelist: Vec::new(),
            box_style: 0,
            box_fill: false,
            box_color: [1.0, 0.2, 0.2, 0.9],
            box_fill_color: [1.0, 0.2, 0.2, 0.15],
            box_fill_opacity: 0.15,
            esp_name_color: [1.0, 1.0, 1.0, 1.0],
            esp_name_size: 12.0,
            esp_name_pos: 0,
            esp_dist_color: [0.8, 0.8, 0.8, 0.9],
            esp_dist_size: 11.0,
            esp_dist_pos: 0,
            esp_weapon_color: [0.9, 0.8, 0.4, 1.0],
            esp_weapon_size: 11.0,
            esp_weapon_pos: 0,
            show_distance_label: false,
            show_equipped_weapon: false,
            team_hide_visuals: true,
            wall_check: false,
            esp_gpu_rendering: false,
        }
    }
}

impl Default for AimbotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: 0,
            fov: 200.0,
            smoothing: 5.0,
            show_fov: true,
            target_bone: "Head".to_string(),
            prediction_enabled: false,
            prediction_amount: 0.02,
            activation_mode: 0,
            hold_delay_ms: 50,
            auto_reload: false,
            prioritize_health: false,
            sens_compensation: false,
            ground_offset_enabled: false,
            ground_offset_y: 1.5,
        }
    }
}


impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            jump_power: 50.0, // default - 50
            walk_speed: 16.0, // default - 16
            fly_enabled: false,
            fly_speed: 25.0,
            noclip_enabled: false,
            auto_jump: false,
            write_intensity: 3, 
            fly_mode: 1, 
            spinbot_enabled: false,
            spinbot_speed: 15.0,
            anti_sit_enabled: false,
            hip_height_enabled: false,
            hip_height_value: 2.0, // default hip height - 2
            void_hide_enabled: false,
        }
    }
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            anti_fog: false,
            fog_start: 10000.0,
            fog_end: 100000.0,
            brightness_enabled: false,
            brightness_value: 2.0,
            anti_flash: false,
            max_brightness: 3.0,
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            fov_enabled: false,
            fov_value: 70.0,
        }
    }
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            show_hotkey_hints: true,
            hotkey_pos_x: 10.0,
            hotkey_pos_y: 10.0,
            hotkey_anchor: "top-left".to_string(),
            expanded_aim_section: 0,
            accent_r: 100,
            accent_g: 100,
            accent_b: 220,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            target_fps: 70,          
            cache_update_ms: 8,  
            idle_mode: false,
            idle_fps: 10,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            menu_toggle: 0x70,        // F1
            reload_data: 0x78,        // F9
            exit: 0x7B,               // F12
            aim_key: 0x02,            // RMB
            autoclicker_toggle: 0x2D, // Insert
            full_refresh: 0x24,       // Home
            save_config: 0x23,        // End
        }
    }
}

impl Default for AutoClickerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: 100.0,
            variance_percent: 15.0,
            hold_duration_ms: 30.0,
            turbo_mode: false,
        }
    }
}

impl Default for HitboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enemy_enabled: false,
            head_scale: 4.0,
            torso_scale: 3.5,
            arms_scale: 3.0,
            legs_scale: 3.0,
            self_enabled: false,
            self_scale: 1.0,
            show_visual: true,
            color: default_hitbox_color(),
        }
    }
}

// Config Manager
pub struct ConfigManager {
    config: Arc<Mutex<Config>>,
    config_path: PathBuf,
}

impl ConfigManager {
    // Create a new config manager, loading from file if it exists.
    pub fn new() -> Self {
        let config_path = Self::get_config_path();
        let config = Self::load_or_default(&config_path);
        
        Self {
            config: Arc::new(Mutex::new(config)),
            config_path,
        }
    }
    
    // Get the config file path.
    fn get_config_path() -> PathBuf {
        let local_config = PathBuf::from("config.toml");
        if local_config.exists() {
            return local_config;
        }
        
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_config = exe_dir.join("config.toml");
                if exe_config.exists() {
                    return exe_config;
                }
            }
        }
        local_config
    }
    
    /// Load config from file or create default if not exists.
    fn load_or_default(path: &PathBuf) -> Config {
        match Self::load_from_file(path) {
            Ok(config) => {
                tracing::info!("Loaded configuration from {:?}", path);
                config
            }
            Err(e) => {
                tracing::warn!("Failed to load config: {}, creating fresh config file", e);
                let config = Config::default();
                
                // Save the default config to create a fresh config.toml
                match toml::to_string_pretty(&config) {
                    Ok(content) => {
                        if let Err(save_err) = fs::write(path, &content) {
                            tracing::error!("Failed to create config file: {}", save_err);
                        } else {
                            tracing::info!("✓ Created fresh config.toml at {:?}", path);
                            tracing::info!("  Edit config.toml and press F9 to reload settings");
                        }
                    }
                    Err(ser_err) => {
                        tracing::error!("Failed to serialize default config: {}", ser_err);
                    }
                }
                
                config
            }
        }
    }
    
    fn load_from_file(path: &PathBuf) -> Result<Config, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save(&self) -> Result<(), ConfigError> {
        let config = self.config.lock().unwrap();
        let content = toml::to_string_pretty(&*config)?;
        fs::write(&self.config_path, content)?;
        tracing::info!("Saved configuration to {:?}", self.config_path);
        Ok(())
    }
    pub fn get(&self) -> Config {
        self.config.lock().unwrap().clone()
    }
    pub fn get_arc(&self) -> Arc<Mutex<Config>> {
        Arc::clone(&self.config)
    }
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Config),
    {
        let mut config = self.config.lock().unwrap();
        f(&mut config);
    }
    pub fn sync(&self, new_config: Config) {
        let mut config = self.config.lock().unwrap();
        *config = new_config;
    }
        pub fn get_username(&self) -> String {
        self.config.lock().unwrap().general.username.clone()
    }
        pub fn get_process_name(&self) -> String {
        self.config.lock().unwrap().general.process_name.clone()
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
