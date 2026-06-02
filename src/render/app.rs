use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{BindableFeature, Config, ConfigManager, HotkeyKey};
use crate::features::{AimAssist, AntiAfk, AutoClicker, AutoReload, CameraAim, Chams, Esp, HitboxExpander, MouseAim, MovementHacks, ViewportAim, WorldModifier};
use crate::features::visuals::EspRenderCache;
use crate::sdk::{Instance, VisualEngine};
use crate::utils::cache::Cache;
use crate::utils::input::Input;

use super::menu::{self, MenuTab, toggle_feature};

pub static PENDING_RELOAD: AtomicBool = AtomicBool::new(false);
pub static PENDING_REFRESH: AtomicBool = AtomicBool::new(false);
pub struct OverlayApp {
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    config_manager: Arc<ConfigManager>,
    local_player_name: String,
    discord_username: String,
    players_instance: Arc<Instance>,
    memory: Arc<crate::core::Memory>,
    datamodel: Arc<Instance>,

    aim_assist: AimAssist,
    camera_aim: CameraAim,
    mouse_aim: MouseAim,
    viewport_aim: ViewportAim,
    esp_render_cache: Arc<EspRenderCache>,
    movement_hacks: MovementHacks,
    world_modifier: WorldModifier,
    autoclicker: AutoClicker,
    hitbox_expander: HitboxExpander,
    anti_afk: AntiAfk,
    auto_reload: AutoReload,
    menu_open: bool,
    menu_minimized: bool,
    current_tab: MenuTab,
    menu_pos: egui::Pos2,
    hotkey_pos: egui::Pos2,
    key_states: [bool; 16],
    last_frame_time: Instant,
    last_reload_time: Instant,
    last_refresh_time: Instant,
    window_initialized: bool,
    win_click_through_style: bool,
}

impl OverlayApp {
    pub fn new(
        cache: Arc<Cache>,
        visengine: Arc<VisualEngine>,
        config_manager: Arc<ConfigManager>,
        memory: Arc<crate::core::Memory>,
        datamodel: Arc<Instance>,
        discord_username: String,
        win_click_through_style: bool,
    ) -> Self {
        let config = config_manager.get();
        let local_player_name = config.general.username.clone();

        let players_instance = datamodel.find_first_child_by_class("Players")
            .map(|p| Arc::new(p))
            .unwrap_or_else(|| Arc::new(Instance::new(0, Arc::clone(&memory))));

        Self {
            cache: Arc::clone(&cache),
            visengine: Arc::clone(&visengine),
            config_manager,
            local_player_name: local_player_name.clone(),
            discord_username,
            players_instance: Arc::clone(&players_instance),
            memory: Arc::clone(&memory),
            datamodel: Arc::clone(&datamodel),
            aim_assist: AimAssist::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            camera_aim: CameraAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            mouse_aim: MouseAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            viewport_aim: ViewportAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine), Arc::clone(&players_instance)),
            esp_render_cache: {
                let esp_cache = Arc::new(EspRenderCache::new());
                EspRenderCache::start(Arc::clone(&esp_cache), Arc::clone(&cache), Arc::clone(&visengine));
                esp_cache
            },
            movement_hacks: MovementHacks::new(Arc::clone(&memory), Arc::clone(&cache), local_player_name.clone()),
            world_modifier: WorldModifier::new(Arc::clone(&memory), &datamodel),
            autoclicker: AutoClicker::new(),
            hitbox_expander: HitboxExpander::new(Arc::clone(&memory), Arc::clone(&cache), local_player_name.clone()),
            anti_afk: AntiAfk::new(),
            auto_reload: {
                let mut ar = AutoReload::new();
                ar.init(Arc::clone(&memory), Arc::clone(&players_instance), local_player_name.clone());
                ar
            },
            menu_open: true,
            menu_minimized: false,
            current_tab: MenuTab::Visuals,
            menu_pos: egui::pos2(20.0, 20.0),
            hotkey_pos: egui::pos2(config.interface.hotkey_pos_x, config.interface.hotkey_pos_y),
            key_states: [false; 16],
            last_frame_time: Instant::now(),
            last_reload_time: Instant::now(),
            last_refresh_time: Instant::now(),
            window_initialized: false,
            win_click_through_style,
        }
    }

    fn handle_hotkeys(&mut self, config: &mut Config) {
        if Input::is_key_pressed(config.hotkeys.menu_toggle as i32, &mut self.key_states[0]) {
            self.menu_open = !self.menu_open;
        }

        if Input::is_key_pressed(config.hotkeys.reload_data as i32, &mut self.key_states[8]) {
            PENDING_RELOAD.store(true, Ordering::SeqCst);
        }

        if Input::is_key_pressed(config.hotkeys.full_refresh as i32, &mut self.key_states[12]) {
            PENDING_REFRESH.store(true, Ordering::SeqCst);
        }

        if Input::is_key_pressed(config.hotkeys.save_config as i32, &mut self.key_states[13]) {
            self.config_manager.save().ok();
            tracing::info!("Config saved to file");
        }

        if Input::is_key_pressed(config.hotkeys.exit as i32, &mut self.key_states[9]) {
            std::process::exit(0);
        }

        if Input::is_key_pressed(config.hotkeys.autoclicker_toggle as i32, &mut self.key_states[14]) {
            self.autoclicker.toggle(&config.autoclicker);
            tracing::info!("AutoClicker: {}", if self.autoclicker.is_running() { "[enabled]" } else { "[disabled]" });
        }

        let bindings: Vec<(usize, HotkeyKey, BindableFeature)> = config.hotkey_bindings.slots.iter()
            .enumerate()
            .filter(|(_, slot)| slot.key != HotkeyKey::None && slot.feature != BindableFeature::None)
            .map(|(idx, slot)| (idx, slot.key, slot.feature))
            .collect();
        
        let reserved_keys = [
            config.hotkeys.menu_toggle,
            config.hotkeys.exit,
            config.hotkeys.reload_data,
            config.hotkeys.full_refresh,
            config.hotkeys.save_config,
        ];
        
        for (slot_idx, key, feature) in bindings {
            let vk_code = key.to_vk_code() as i32;
            let state_idx = slot_idx + 1;
            
            if reserved_keys.contains(&(vk_code as u32)) {
                continue;
            }
            
            if state_idx < 15 && Input::is_key_pressed(vk_code, &mut self.key_states[state_idx]) {
                let feature_name = feature.display_name();
                toggle_feature(config, feature);
                
                // Log the toggle
                let is_enabled = match feature {
                    BindableFeature::BoxEsp => config.visuals.box_esp,
                    BindableFeature::NameTags => config.visuals.name_tags,
                    BindableFeature::Tracers => config.visuals.tracers,
                    BindableFeature::HealthBars => config.visuals.health_bars,
                    BindableFeature::ArmourBars => config.visuals.armor_bars,
                    BindableFeature::Chams => config.visuals.chams,
                    BindableFeature::TeamCheck => config.visuals.team_check,
                    BindableFeature::HideDead => config.visuals.hide_dead,
                    BindableFeature::ShowBots => config.visuals.show_bots,
                    BindableFeature::AimAssist => config.aimbot.enabled,
                    BindableFeature::Triggerbot => false,
                    BindableFeature::CameraAim => config.camera_aim.enabled,
                    BindableFeature::AutoReload => config.aimbot.auto_reload,
                    BindableFeature::Fly => config.movement.fly_enabled,
                    BindableFeature::Noclip => config.movement.noclip_enabled,
                    BindableFeature::Spinbot => config.movement.spinbot_enabled,
                    BindableFeature::AntiSit => config.movement.anti_sit_enabled,
                    BindableFeature::VoidHide => config.movement.void_hide_enabled,
                    BindableFeature::HitboxMod => config.hitbox.enabled,
                    BindableFeature::ShowHitboxVisual => config.hitbox.show_visual,
                    _ => false,
                };
                tracing::info!("{}: {}", feature_name, if is_enabled { "[enabled]" } else { "[disabled]" });
            }
        }
    }

    fn reload_data(&mut self) {
        if self.last_reload_time.elapsed().as_secs() >= 2 {
            self.cache.clear();
            self.last_reload_time = Instant::now();
        }
    }

    fn refresh_game_instances(&mut self) {
        if self.last_refresh_time.elapsed().as_secs() < 3 {
            tracing::warn!("Refresh on cooldown, please wait...");
            return;
        }

        tracing::info!("Refreshing game instances...");
        
        let config = self.config_manager.get();
        let base = self.memory.base_address();

        let fake_dm = self.memory.read::<u64>(base + crate::core::offsets::fake_datamodel::pointer());
        if fake_dm == 0 {
            tracing::error!("FakeDataModel is null - game may not be running");
            return;
        }

        let dm_addr = self.memory.read::<u64>(fake_dm + crate::core::offsets::fake_datamodel::real_datamodel());
        if dm_addr == 0 {
            tracing::error!("DataModel is null - game may be loading");
            return;
        }

        let new_datamodel = Arc::new(Instance::new(dm_addr, Arc::clone(&self.memory)));

        let ve_addr = self.memory.read::<u64>(base + crate::core::offsets::visual_engine::pointer());
        let new_visengine = Arc::new(crate::sdk::VisualEngine::new(ve_addr, Arc::clone(&self.memory)));

        let new_players = match new_datamodel.find_first_child_by_class("Players") {
            Some(p) => Arc::new(p),
            None => {
                tracing::error!("Players service not found - game may be loading");
                return;
            }
        };

        self.datamodel = Arc::clone(&new_datamodel);
        self.visengine = Arc::clone(&new_visengine);
        self.players_instance = Arc::clone(&new_players);

        self.aim_assist = AimAssist::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.camera_aim = CameraAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.mouse_aim = MouseAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.viewport_aim = ViewportAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine), Arc::clone(&new_players));
        self.world_modifier = WorldModifier::new(Arc::clone(&self.memory), &self.datamodel);
        self.hitbox_expander = HitboxExpander::new(
            Arc::clone(&self.memory),
            Arc::clone(&self.cache),
            self.local_player_name.clone(),
        );

        let new_workspace = match new_datamodel.find_first_child_by_class("Workspace") {
            Some(w) => Arc::new(w),
            None => {
                tracing::error!("Workspace not found - game may be loading");
                return;
            }
        };

        self.cache.restart(
            new_players,
            new_workspace,
            Arc::clone(&self.memory),
            config.performance.cache_update_ms,
        );

        self.last_refresh_time = Instant::now();
        tracing::info!("Game instances refreshed successfully");
    }

    fn setup_window(&mut self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                FindWindowW, GetWindowLongW, SetWindowLongW, SetWindowPos,
                GWL_EXSTYLE, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE,
                WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            };

            let config = self.config_manager.get();
            let title: Vec<u16> = config.general.window_title.encode_utf16().chain(std::iter::once(0)).collect();

            if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR::from_raw(title.as_ptr())) {
                if !self.window_initialized {
                    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    let mut new_style = ex_style | WS_EX_LAYERED.0 as i32 | WS_EX_TOOLWINDOW.0 as i32;
                    if self.win_click_through_style {
                        // Enable Windows-level click-through (in addition to egui mouse passthrough)
                        new_style |= WS_EX_TRANSPARENT.0 as i32;
                    }
                    SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
                    
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    
                    self.window_initialized = true;
                }
            }
        }
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut config = self.config_manager.get();

        if PENDING_RELOAD.swap(false, Ordering::SeqCst) {
            self.reload_data();
        }

        if PENDING_REFRESH.swap(false, Ordering::SeqCst) {
            self.refresh_game_instances();
        }

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::TRANSPARENT;
        visuals.window_fill = egui::Color32::TRANSPARENT;
        visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
        visuals.faint_bg_color = egui::Color32::TRANSPARENT;
        visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
        ctx.set_visuals(visuals);

        self.handle_hotkeys(&mut config);
        self.setup_window();
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!self.menu_open));

        let target_fps = if self.menu_open || config.visuals.box_esp || config.visuals.chams || config.aimbot.enabled {
            config.performance.target_fps
        } else if config.performance.idle_mode {
            config.performance.idle_fps
        } else {
            config.performance.target_fps
        };

        let frame_delay = Duration::from_secs_f64(1.0 / target_fps.clamp(5, 144) as f64);
        ctx.request_repaint_after(frame_delay);
        self.last_frame_time = Instant::now();

        self.movement_hacks.apply_all(&config, self.menu_open);
        self.world_modifier.apply_all(&config);
        self.anti_afk.update(&config.anti_afk);
        
        if !self.menu_open {
            self.auto_reload.update(config.aimbot.auto_reload);
        }
        
        self.cache.set_show_bots(config.visuals.show_bots);
        self.hitbox_expander.update(&config.hitbox);

        if config.aimbot.enabled {
            self.aim_assist.apply(&config, &self.local_player_name, None);
        }

        if config.camera_aim.enabled {
            self.camera_aim.update(&config, &self.local_player_name, None);
        }

        if config.mouse_aim.enabled {
            self.mouse_aim.update(&config, &self.local_player_name, None);
        }

        if config.viewport_aim.enabled {
            let camera_addr = self.memory.resolve_camera_address().unwrap_or(0);
            self.viewport_aim.update(&config, &self.local_player_name, camera_addr, None);
        }

        if config.visuals.box_esp || config.visuals.name_tags || config.visuals.tracers {
            let aim_target_name: String = self.aim_assist.get_current_target_name()
                .or_else(|| self.camera_aim.get_current_target_name())
                .or_else(|| self.mouse_aim.get_current_target_name())
                .or_else(|| self.viewport_aim.get_current_target_name())
                .unwrap_or("")
                .to_string();
            self.esp_render_cache.update_config(&config, &self.local_player_name, &aim_target_name);
            Esp::render_cached(
                ctx,
                &self.esp_render_cache,
                &config,
                &self.visengine,
            );
        }

        if config.visuals.chams {
            Chams::render(
                ctx,
                &self.cache,
                &self.visengine,
                &config,
                None,
                &self.local_player_name,
            );
        }

        if config.hitbox.enabled && config.hitbox.show_visual {
            Esp::render_hitbox_visual(
                ctx,
                &self.cache,
                &self.visengine,
                &config,
                &self.local_player_name,
            );
        }

        // Draw FOV circles for any aim system that has show_fov enabled
        let any_fov_visible = (config.aimbot.enabled && config.aimbot.show_fov)
            || (config.camera_aim.enabled && config.camera_aim.show_fov)
            || (config.mouse_aim.enabled && config.mouse_aim.show_fov)
            || (config.viewport_aim.enabled && config.viewport_aim.show_fov);
        if any_fov_visible {
            Esp::render_fov_circle(ctx, &config, &self.visengine);
        }

        if self.menu_open {
            self.hotkey_pos = egui::pos2(config.interface.hotkey_pos_x, config.interface.hotkey_pos_y);
        }

        menu::render_hotkey_hints(ctx, &mut self.hotkey_pos, self.menu_open, &mut config);

        if self.menu_open {
            menu::render_menu(
                ctx,
                &mut self.menu_pos,
                &mut self.menu_minimized,
                &mut self.current_tab,
                &mut config,
                &self.cache,
                &mut self.autoclicker,
                &mut self.movement_hacks,
                &self.discord_username,
            );
        }

        self.config_manager.sync(config.clone());
        self.autoclicker.update_recording();
        self.config_manager.update(|c| *c = config);
    }
}
