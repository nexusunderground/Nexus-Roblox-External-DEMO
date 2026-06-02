use eframe::egui;
use std::sync::Arc;

use crate::config::Config;
use crate::features::{autoclicker::AutoClicker, movement::MovementHacks};
use crate::utils::cache::Cache;

use super::theme;
use super::widgets;

#[derive(PartialEq, Clone, Copy)]
pub enum MenuTab {
    Visuals,
    Aimbot,
    Movement,
    World,
    AutoClicker,
    Hitbox,
    Hotkeys,
    Misc,
    Performance,
    About,
}


pub fn render_menu(
    ctx: &egui::Context,
    menu_pos: &mut egui::Pos2,
    menu_minimized: &mut bool,
    current_tab: &mut MenuTab,
    config: &mut Config,
    cache: &Arc<Cache>,
    autoclicker: &mut AutoClicker,
    _movement_hacks: &mut MovementHacks,
    discord_username: &str,
) {
    let menu_width = 340.0;

    let menu_response = egui::Area::new(egui::Id::new("nexus_menu"))
        .current_pos(*menu_pos)
        .movable(true)
        .constrain(true)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
            
            egui::Frame::none()
                .fill(theme::BG_DARK)
                .rounding(6.0)
                .stroke(egui::Stroke::new(2.0, accent))
                .inner_margin(egui::Margin::same(2.0))
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(theme::BG_DARK)
                        .rounding(4.0)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                        .show(ui, |ui| {
                    ui.set_width(menu_width);

                    render_header(ui, menu_minimized);

                    if !*menu_minimized {
                        render_tab_bar(ui, current_tab);

                        ui.add_space(1.0);
                        let sep_rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            sep_rect.left()..=sep_rect.right(),
                            sep_rect.top(),
                            egui::Stroke::new(1.0, theme::BORDER_DEFAULT),
                        );
                        ui.add_space(1.0);

                        egui::Frame::none()
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(350.0)
                                    .show(ui, |ui| match *current_tab {
                                        MenuTab::Visuals => render_visuals_tab(ui, config),
                                        MenuTab::Aimbot => render_aimbot_tab(ui, config),
                                        MenuTab::Movement => render_movement_tab(ui, config),
                                        MenuTab::World => render_world_tab(ui, config),
                                        MenuTab::AutoClicker => render_autoclicker_tab(ui, config, autoclicker),
                                        MenuTab::Hitbox => render_hitbox_tab(ui, config),
                                        MenuTab::Hotkeys => render_hotkeys_tab(ui, config),
                                        MenuTab::Misc => render_misc_tab(ui, config, cache),
                                        MenuTab::Performance => render_performance_tab(ui, config),
                                        MenuTab::About => render_about_tab(ui, config),
                                    });
                            });

                        render_footer(ui, cache, discord_username);
                    }
                        });
                });
        });

    if menu_response.response.dragged() {
        *menu_pos = ctx.input(|i| *menu_pos + i.pointer.delta());
    }
}

use crate::config::BindableFeature;

fn get_feature_state(config: &Config, feature: BindableFeature) -> bool {
    match feature {
        BindableFeature::None => false,
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
    }
}


pub fn toggle_feature(config: &mut Config, feature: BindableFeature) {
    match feature {
        BindableFeature::None => {},
        BindableFeature::BoxEsp => config.visuals.box_esp = !config.visuals.box_esp,
        BindableFeature::NameTags => config.visuals.name_tags = !config.visuals.name_tags,
        BindableFeature::Tracers => config.visuals.tracers = !config.visuals.tracers,
        BindableFeature::HealthBars => config.visuals.health_bars = !config.visuals.health_bars,
        BindableFeature::ArmourBars => config.visuals.armor_bars = !config.visuals.armor_bars,
        BindableFeature::Chams => config.visuals.chams = !config.visuals.chams,
        BindableFeature::TeamCheck => config.visuals.team_check = !config.visuals.team_check,
        BindableFeature::HideDead => config.visuals.hide_dead = !config.visuals.hide_dead,
        BindableFeature::ShowBots => config.visuals.show_bots = !config.visuals.show_bots,
        BindableFeature::AimAssist => config.aimbot.enabled = !config.aimbot.enabled,
        BindableFeature::Triggerbot => {},
        BindableFeature::CameraAim => config.camera_aim.enabled = !config.camera_aim.enabled,
        BindableFeature::AutoReload => config.aimbot.auto_reload = !config.aimbot.auto_reload,
        BindableFeature::Fly => config.movement.fly_enabled = !config.movement.fly_enabled,
        BindableFeature::Noclip => config.movement.noclip_enabled = !config.movement.noclip_enabled,
        BindableFeature::Spinbot => config.movement.spinbot_enabled = !config.movement.spinbot_enabled,
        BindableFeature::AntiSit => config.movement.anti_sit_enabled = !config.movement.anti_sit_enabled,
        BindableFeature::VoidHide => config.movement.void_hide_enabled = !config.movement.void_hide_enabled,
        BindableFeature::HitboxMod => config.hitbox.enabled = !config.hitbox.enabled,
        BindableFeature::ShowHitboxVisual => config.hitbox.show_visual = !config.hitbox.show_visual,
    }
}


pub fn render_hotkey_hints(ctx: &egui::Context, hotkey_pos: &mut egui::Pos2, menu_open: bool, config: &mut Config) {
    if !config.interface.show_hotkey_hints || menu_open {
        return;
    }

    let hotkey_response = egui::Area::new(egui::Id::new("hotkey_hints"))
        .current_pos(*hotkey_pos)
        .movable(true)
        .constrain(true)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 20, 240))
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .rounding(3.0)
                .show(ui, |ui| {
                    ui.set_width(110.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("hotkeys").size(10.0).color(theme::TEXT_SECONDARY));
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("F1").size(8.0).color(theme::TEXT_MUTED));
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Menu").size(9.0).color(theme::TEXT_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let size = egui::vec2(8.0, 8.0);
                            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                            ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
                            let inner = rect.shrink(2.0);
                            ui.painter().rect_filled(inner, 0.0, theme::ACCENT_PRIMARY);
                        });
                    });

                    ui.spacing_mut().item_spacing.y = 1.0;
                    for slot in &config.hotkey_bindings.slots {
                        if slot.feature == BindableFeature::None || slot.key == crate::config::HotkeyKey::None {
                            continue;
                        }
                        
                        let key_name = slot.key.display_name();
                        let feature_name = slot.feature.display_name();
                        let active = get_feature_state(config, slot.feature);
                        
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(key_name).size(8.0).color(theme::TEXT_MUTED));
                            ui.add_space(4.0);
                            let display_name = if feature_name.len() > 10 {
                                &feature_name[..10]
                            } else {
                                feature_name
                            };
                            ui.label(egui::RichText::new(display_name).size(9.0).color(theme::TEXT_SECONDARY));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let size = egui::vec2(8.0, 8.0);
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
                                if active {
                                    let inner = rect.shrink(2.0);
                                    ui.painter().rect_filled(inner, 0.0, theme::ACCENT_PRIMARY);
                                }
                            });
                        });

                        // Show fly mode when fly is enabled
                        if slot.feature == BindableFeature::Fly && config.movement.fly_enabled {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                let mode_text = match config.movement.fly_mode {
                                    0 => "vel",
                                    _ => "stable",
                                };
                                ui.label(egui::RichText::new(format!("└ {}", mode_text))
                                    .size(8.0)
                                    .color(theme::TEXT_MUTED));
                            });
                        }
                    }
                });
        });

    if hotkey_response.response.dragged() {
        *hotkey_pos = ctx.input(|i| *hotkey_pos + i.pointer.delta());
        config.interface.hotkey_pos_x = hotkey_pos.x;
        config.interface.hotkey_pos_y = hotkey_pos.y;
    }
}

fn render_header(ui: &mut egui::Ui, _menu_minimized: &mut bool) {
    egui::Frame::none()
        .fill(theme::BG_MEDIUM)
        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("[DEMO]").size(12.0).color(theme::TEXT_MUTED));
                
                ui.add_space(4.0);
                
                let time = ui.ctx().input(|i| i.time);
                let pulse = ((time * 2.5 + (time * 7.3).sin() * 0.5).sin() * 0.5 + 0.5) as f32;
                let glow_alpha = (pulse * 200.0 + 55.0) as u8;
                let glow_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, glow_alpha);
                
                let text_response = ui.label(egui::RichText::new("nexus").size(12.0).color(glow_color).strong());
                let glow_rect = text_response.rect.expand(2.0 + pulse * 2.0);
                ui.painter().rect_filled(glow_rect, 4.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, (pulse * 30.0) as u8));
                
                ui.label(egui::RichText::new("underground").size(10.0).color(theme::ACCENT_PRIMARY));
                ui.ctx().request_repaint();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(window_button("×", theme::ACCENT_DANGER)).on_hover_text("Exit (F12)").clicked() {
                        std::process::exit(0);
                    }
                });
            });
        });
}

fn window_button(icon: &str, color: egui::Color32) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(icon).size(14.0).color(color))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .rounding(2.0)
        .min_size(egui::vec2(20.0, 20.0))
}

fn render_tab_bar(ui: &mut egui::Ui, current_tab: &mut MenuTab) {
    egui::Frame::none()
        .fill(theme::BG_DARK)
        .inner_margin(egui::Margin::symmetric(4.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().item_spacing.y = 2.0;

                let tabs = [
                    ("visuals", MenuTab::Visuals),
                    ("aim", MenuTab::Aimbot),
                    ("movement", MenuTab::Movement),
                    ("world", MenuTab::World),
                    ("clicker", MenuTab::AutoClicker),
                    ("hitbox", MenuTab::Hitbox),
                    ("hotkeys", MenuTab::Hotkeys),
                    ("misc", MenuTab::Misc),
                    ("config", MenuTab::Performance),
                    ("about", MenuTab::About),
                ];

                for (label, tab) in tabs {
                    let active = *current_tab == tab;

                    let btn = egui::Button::new(
                        egui::RichText::new(label)
                            .size(10.0)
                            .color(if active { theme::TEXT_PRIMARY } else { theme::TEXT_MUTED }),
                    )
                    .fill(if active { theme::BG_LIGHT } else { egui::Color32::TRANSPARENT })
                    .stroke(if active { 
                        egui::Stroke::new(1.0, theme::BORDER_FRAME) 
                    } else { 
                        egui::Stroke::NONE 
                    })
                    .rounding(2.0)
                    .min_size(egui::vec2(40.0, 18.0));

                    if ui.add(btn).clicked() {
                        *current_tab = tab;
                    }
                }
            });
        });
}

fn render_footer(ui: &mut egui::Ui, cache: &Arc<Cache>, discord_username: &str) {
    ui.add_space(2.0);
    let sep_rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        sep_rect.left()..=sep_rect.right(),
        sep_rect.top(),
        egui::Stroke::new(1.0, theme::BORDER_DEFAULT),
    );

    egui::Frame::none()
        .fill(theme::BG_MEDIUM)
        .rounding(egui::Rounding { nw: 0.0, ne: 0.0, sw: 4.0, se: 4.0 })
        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let count = cache.count();
                ui.label(
                    egui::RichText::new(format!("{} players", count))
                        .size(10.0)
                        .color(if count > 0 { theme::ACCENT_PRIMARY } else { theme::TEXT_MUTED }),
                );
                
                if !discord_username.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(discord_username).size(9.0).color(theme::ACCENT_INFO));
                    });
                }
            });
            
            ui.add_space(4.0);
            
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("[F1] MENU").size(9.0).color(egui::Color32::from_rgb(100, 150, 255)).strong());
                ui.label(egui::RichText::new("•").size(9.0).color(theme::BORDER_DEFAULT));
                ui.label(egui::RichText::new("[F12] EXIT").size(9.0).color(egui::Color32::from_rgb(255, 80, 80)).strong());
                ui.label(egui::RichText::new("•").size(9.0).color(theme::BORDER_DEFAULT));
                ui.label(egui::RichText::new("[HOME] REFRESH").size(9.0).color(egui::Color32::from_rgb(255, 180, 50)).strong());
                ui.label(egui::RichText::new("•").size(9.0).color(theme::BORDER_DEFAULT));
                ui.label(egui::RichText::new("[END] SAVE").size(9.0).color(egui::Color32::from_rgb(100, 255, 100)).strong());
            });
        });
}

fn render_visuals_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "esp", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.box_esp, "box esp", Some("F2"));
        widgets::styled_toggle(ui, &mut config.visuals.name_tags, "name tags", None);
        widgets::styled_toggle(ui, &mut config.visuals.tracers, "tracers", Some("F8"));
        widgets::styled_toggle(ui, &mut config.visuals.health_bars, "health bars", None);
        widgets::styled_toggle(ui, &mut config.visuals.armor_bars, "armor bars", None);
    });

    ui.add_space(4.0);
        widgets::double_border_frame(ui, "effects", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.chams, "chams glow", Some("F3"));
    });

    ui.add_space(4.0);
        widgets::double_border_frame(ui, "camera", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.camera.fov_enabled, "fov changer", None);
        if config.camera.fov_enabled {
            widgets::styled_slider(ui, "fov", &mut config.camera.fov_value, 1.0..=120.0, "°");
            config.camera.fov_value = config.camera.fov_value.clamp(1.0, 120.0);
        }
    });

    ui.add_space(4.0);
        widgets::double_border_frame(ui, "FILTERS", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.team_check, "team check", None);
        if config.visuals.team_check {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("add teammate name:").size(9.0).color(theme::TEXT_MUTED));
            
            ui.horizontal(|ui| {
                let input_id = ui.id().with("team_input");
                let mut input_text = ui.ctx().data_mut(|d| {
                    d.get_persisted::<String>(input_id).unwrap_or_default()
                });
                
                let text_edit = egui::TextEdit::singleline(&mut input_text)
                    .desired_width(ui.available_width() - 50.0)
                    .font(egui::TextStyle::Small)
                    .hint_text("Player name...");
                
                let response = ui.add(text_edit);
                
                let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let add_clicked = ui.add(
                    egui::Button::new(egui::RichText::new("+").size(12.0).color(theme::ACCENT_SUCCESS))
                        .fill(theme::BG_DARK)
                        .min_size(egui::vec2(30.0, 20.0))
                ).clicked();
                
                if (enter_pressed || add_clicked) && !input_text.trim().is_empty() {
                    let name = input_text.trim().to_string();
                    if !config.visuals.teammate_whitelist.contains(&name) {
                        config.visuals.teammate_whitelist.push(name);
                    }
                    input_text.clear();
                }
                
                ui.ctx().data_mut(|d| d.insert_persisted(input_id, input_text));
            });
            
            if !config.visuals.teammate_whitelist.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("teammates ({}):", config.visuals.teammate_whitelist.len())).size(8.0).color(theme::ACCENT_INFO));
                
                let mut to_remove: Option<usize> = None;
                for (idx, name) in config.visuals.teammate_whitelist.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("• {}", name)).size(9.0).color(theme::TEXT_PRIMARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(egui::RichText::new("×").size(10.0).color(theme::ACCENT_DANGER))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .min_size(egui::vec2(16.0, 16.0))
                            ).clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    });
                }
                if let Some(idx) = to_remove {
                    config.visuals.teammate_whitelist.remove(idx);
                }
            }
        }
        
        widgets::styled_toggle(ui, &mut config.visuals.hide_dead, "hide dead", None);
        widgets::styled_toggle(ui, &mut config.visuals.show_bots, "show bots/npcs", None);
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "display", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.distance_colors, "distance colors", None);
        widgets::styled_toggle(ui, &mut config.visuals.target_highlight, "target highlight", None);
        widgets::styled_slider(ui, "max distance", &mut config.visuals.max_distance, 50.0..=1000.0, "m");
    });
}

fn render_aimbot_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "GLOBAL SETTINGS", accent, |ui| {
        ui.label(egui::RichText::new("(applies to all aim systems)").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        
        widgets::styled_toggle(ui, &mut config.aimbot.prediction_enabled, "prediction", None);
        if config.aimbot.prediction_enabled {
            widgets::styled_slider(ui, "lead time", &mut config.aimbot.prediction_amount, 0.01..=0.2, "s");
        }
    });
    
    ui.add_space(6.0);

    if widgets::aim_section_header(
        ui,
        "aim assist",
        Some("humanized guidance for locking onto targets"),
        &mut config.aimbot.enabled,
        1,
        &mut config.interface.expanded_aim_section,
        Some("F4"),
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new("hold [RMB] to aim").size(9.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                
                widgets::styled_slider(ui, "fov radius", &mut config.aimbot.fov, 20.0..=500.0, "px");
                widgets::styled_slider(ui, "smoothing", &mut config.aimbot.smoothing, 1.0..=40.0, "");
                widgets::styled_toggle(ui, &mut config.aimbot.show_fov, "show fov", None);
                
                ui.add_space(4.0);
                widgets::bone_selector(ui, "target", &mut config.aimbot.target_bone, "bone_select");
                
                ui.add_space(4.0);
                widgets::activation_mode_selector(ui, "activation", &mut config.aimbot.activation_mode, "activation_mode_select");
                
                if config.aimbot.activation_mode == 0 {
                    ui.add_space(2.0);
                    let mut hold_delay_f = config.aimbot.hold_delay_ms as f32;
                    widgets::styled_slider(ui, "hold delay", &mut hold_delay_f, 0.0..=500.0, "ms");
                    config.aimbot.hold_delay_ms = hold_delay_f as u32;
                }
            });
    }

    if widgets::aim_section_header(
        ui,
        "mouse aim",
        Some("SendInput cursor movement"),
        &mut config.mouse_aim.enabled,
        2,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new("hold [RMB] to aim").size(9.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                widgets::styled_slider(ui, "fov radius", &mut config.mouse_aim.fov, 20.0..=500.0, "px");
                widgets::styled_slider(ui, "smoothing", &mut config.mouse_aim.smoothing, 1.0..=40.0, "");
                widgets::styled_toggle(ui, &mut config.mouse_aim.show_fov, "show fov", None);
                ui.add_space(4.0);
                widgets::bone_selector(ui, "target", &mut config.mouse_aim.target_bone, "mouse_bone_select");
            });
    }

    if widgets::aim_section_header(
        ui,
        "camera aim",
        Some("spoofs camera CFrame rotation"),
        &mut config.camera_aim.enabled,
        3,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                widgets::styled_slider(ui, "fov", &mut config.camera_aim.fov, 10.0..=400.0, "px");
                widgets::styled_toggle(ui, &mut config.camera_aim.show_fov, "show fov", None);
                ui.add_space(4.0);
                widgets::bone_selector(ui, "target", &mut config.camera_aim.target_bone, "cam_bone_select");
            });
    }

    if widgets::aim_section_header(
        ui,
        "viewport aim",
        Some("writes target offset to camera viewport"),
        &mut config.viewport_aim.enabled,
        4,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                widgets::styled_slider(ui, "fov", &mut config.viewport_aim.fov, 10.0..=400.0, "px");
                widgets::styled_toggle(ui, &mut config.viewport_aim.show_fov, "show fov", None);
            });
    }

    ui.add_space(4.0);
    widgets::section_header(ui, "auto reload");
    widgets::styled_toggle(ui, &mut config.aimbot.auto_reload, "auto reload", None);
}

fn render_movement_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "MOVEMENT", accent, |ui| {
        widgets::editable_slider(ui, "jump power", &mut config.movement.jump_power, 50.0..=300.0, "", "jump_power");
        widgets::styled_toggle(ui, &mut config.movement.auto_jump, "auto jump", None);
        widgets::editable_slider(ui, "walk speed", &mut config.movement.walk_speed, 16.0..=500.0, "", "walk_speed");
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "FLY", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.movement.fly_enabled, "enabled", Some("F6"));
        if config.movement.fly_enabled {
            widgets::editable_slider(ui, "speed", &mut config.movement.fly_speed, 10.0..=500.0, "", "fly_speed");
            widgets::fly_mode_selector(ui, "mode", &mut config.movement.fly_mode, "fly_mode");
            ui.add_space(4.0);
            ui.label(egui::RichText::new("writes per second:").size(9.0).color(theme::TEXT_MUTED));
            widgets::write_intensity_selector(ui, &mut config.movement.write_intensity);
            ui.add_space(4.0);
            ui.label(egui::RichText::new("TIP: Active once menu is closed").size(8.0).color(theme::ACCENT_INFO));
            ui.label(egui::RichText::new("SPACE = ascend | CTRL = descend").size(8.0).color(theme::ACCENT_INFO));
        }
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "OTHER", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.movement.noclip_enabled, "noclip", Some("F10"));
        widgets::styled_toggle(ui, &mut config.movement.spinbot_enabled, "spinbot", None);
        if config.movement.spinbot_enabled {
            widgets::styled_slider(ui, "spin speed", &mut config.movement.spinbot_speed, 1.0..=30.0, "°");
        }
        widgets::styled_toggle(ui, &mut config.movement.anti_sit_enabled, "anti-sit", None);
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "MISC", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.movement.hip_height_enabled, "hip height", None);
        if config.movement.hip_height_enabled {
            widgets::editable_slider(ui, "height", &mut config.movement.hip_height_value, 0.0..=1300.0, "", "hip_height");
        }
        widgets::styled_toggle(ui, &mut config.movement.void_hide_enabled, "void hide", None);
        if config.movement.void_hide_enabled {
            ui.label(egui::RichText::new("TIP: Teleports off map to avoid hits").size(8.0).color(theme::ACCENT_INFO));
        }
    });
}

fn render_world_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "FOG", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.anti_fog, "anti-fog", None);
        if config.world.anti_fog {
            widgets::styled_slider(ui, "start", &mut config.world.fog_start, 0.0..=100000.0, "");
            widgets::styled_slider(ui, "end", &mut config.world.fog_end, 0.0..=100000.0, "");
        }
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "LIGHTING", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.brightness_enabled, "brightness", None);
        if config.world.brightness_enabled {
            widgets::styled_slider(ui, "value", &mut config.world.brightness_value, 0.0..=10.0, "");
        }
        widgets::styled_toggle(ui, &mut config.world.anti_flash, "anti-flash", None);
        if config.world.anti_flash {
            widgets::styled_slider(ui, "max", &mut config.world.max_brightness, 0.5..=5.0, "");
        }
    });
}

fn render_autoclicker_tab(ui: &mut egui::Ui, config: &mut Config, autoclicker: &mut AutoClicker) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    let is_running = autoclicker.is_running();
    let state_arc = autoclicker.state();
    let state = state_arc.lock().unwrap();
    let sequence_len = state.sequence.len();
    let is_recording = state.recording;
    let current_index = state.current_index;
    let total_clicks = state.total_clicks;
    let sequence_display: Vec<String> = state.sequence.iter().map(|b| b.display_name()).collect();
    drop(state);

    widgets::double_border_frame(ui, "HOW TO USE", accent, |ui| {
        ui.label(egui::RichText::new("1. Click 'Record Buttons' to start").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("2. Press mouse buttons to record sequence").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("3. Click 'Stop Recording' when done").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("4. Press [Insert] to start/stop clicking").size(9.0).color(theme::ACCENT_PRIMARY));
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "STATUS", accent, |ui| {
        ui.horizontal(|ui| {
            let (status_text, status_color) = if is_running {
                ("Running", theme::ACCENT_SUCCESS)
            } else if is_recording {
                ("Recording", theme::ACCENT_WARNING)
            } else {
                ("Stopped", theme::TEXT_MUTED)
            };
            ui.label(egui::RichText::new(status_text).size(11.0).color(status_color).strong());
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("Clicks: {}", total_clicks)).size(9.0).color(theme::TEXT_SECONDARY));
            });
        });
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "TIMING", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.autoclicker.turbo_mode, "Turbo Mode", Some("Maximum speed, no delays"));
        
        if !config.autoclicker.turbo_mode {
            widgets::styled_slider(ui, "Delay", &mut config.autoclicker.delay_ms, 10.0..=1000.0, "ms");
            widgets::styled_slider(ui, "Variance", &mut config.autoclicker.variance_percent, 0.0..=50.0, "%");
            widgets::styled_slider(ui, "Hold", &mut config.autoclicker.hold_duration_ms, 10.0..=200.0, "ms");
        }
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "SEQUENCE", accent, |ui| {
        if sequence_display.is_empty() {
            ui.label(egui::RichText::new("No buttons recorded").size(9.0).color(theme::TEXT_MUTED).italics());
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (idx, name) in sequence_display.iter().enumerate() {
                    let is_current = is_running && idx == current_index;
                    let bg = if is_current { theme::ACCENT_PRIMARY } else { theme::BG_LIGHT };
                    let text_color = if is_current { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY };
                    
                    egui::Frame::none()
                        .fill(bg)
                        .rounding(3.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(name).size(9.0).color(text_color));
                        });
                }
            });
        }
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "CONTROLS", accent, |ui| {
        ui.horizontal(|ui| {
            let record_text = if is_recording { "Stop Recording" } else { "Record Buttons" };
            let record_color = if is_recording { theme::ACCENT_WARNING } else { theme::TEXT_PRIMARY };
            
            let record_btn = egui::Button::new(egui::RichText::new(record_text).size(10.0).color(record_color))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, if is_recording { theme::ACCENT_WARNING } else { theme::BORDER_DEFAULT }))
                .rounding(4.0)
                .min_size(egui::vec2(ui.available_width() * 0.65, 18.0));
            
            if ui.add_enabled(!is_running, record_btn).clicked() {
                if is_recording {
                    autoclicker.stop_recording();
                } else {
                    autoclicker.start_recording();
                }
            }
            
            let clear_btn = egui::Button::new(egui::RichText::new("Clear").size(10.0).color(theme::TEXT_SECONDARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0);
            
            if ui.add_enabled(!is_running && !is_recording && sequence_len > 0, clear_btn).clicked() {
                autoclicker.clear_sequence();
            }
        });

        if !is_running && !is_recording && sequence_len > 0 {
            if ui.add(
                egui::Button::new(egui::RichText::new("Remove Last").size(9.0).color(theme::TEXT_MUTED))
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
            ).clicked() {
                autoclicker.remove_last();
            }
        }

        ui.add_space(4.0);
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Press").size(9.0).color(theme::TEXT_MUTED));
            ui.label(egui::RichText::new("F10").size(9.0).color(theme::ACCENT_PRIMARY).strong());
            ui.label(egui::RichText::new("to toggle").size(9.0).color(theme::TEXT_MUTED));
        });
    });
}

fn render_hitbox_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "HITBOX MODIFIER", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.hitbox.enabled, "Enable Hitbox Mod", Some("F11"));
        ui.label(egui::RichText::new("Modifies primitive sizes").size(9.0).color(theme::ACCENT_WARNING));
        
        ui.add_space(4.0);
        widgets::styled_toggle(ui, &mut config.hitbox.show_visual, "Show Hitbox Visual", None);
        if config.hitbox.show_visual {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Color").size(10.0).color(theme::TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut color = egui::Color32::from_rgba_unmultiplied(
                        (config.hitbox.color[0] * 255.0) as u8,
                        (config.hitbox.color[1] * 255.0) as u8,
                        (config.hitbox.color[2] * 255.0) as u8,
                        (config.hitbox.color[3] * 255.0) as u8,
                    );
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        config.hitbox.color = [
                            color.r() as f32 / 255.0,
                            color.g() as f32 / 255.0,
                            color.b() as f32 / 255.0,
                            color.a() as f32 / 255.0,
                        ];
                    }
                });
            });
        }
    });

    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "ENEMY HITBOXES", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.hitbox.enemy_enabled, "expand enemy hitboxes", None);
        ui.label(egui::RichText::new("easier to hit them").size(9.0).color(theme::TEXT_MUTED));
        
        if config.hitbox.enemy_enabled {
            ui.add_space(4.0);
            widgets::styled_slider(ui, "Head", &mut config.hitbox.head_scale, 1.0..=5.0, "x");
            widgets::styled_slider(ui, "Torso", &mut config.hitbox.torso_scale, 1.0..=4.0, "x");
            widgets::styled_slider(ui, "Arms", &mut config.hitbox.arms_scale, 1.0..=3.0, "x");
            widgets::styled_slider(ui, "Legs", &mut config.hitbox.legs_scale, 1.0..=3.0, "x");
        }
    });
    
    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "YOUR HITBOX", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.hitbox.self_enabled, "shrink your hitbox", None);
        ui.label(egui::RichText::new("harder to hit you").size(9.0).color(theme::TEXT_MUTED));
        
        if config.hitbox.self_enabled {
            ui.add_space(4.0);
            widgets::styled_slider(ui, "Self Scale", &mut config.hitbox.self_scale, 0.1..=2.0, "x");
        }
    });


}

fn render_hotkeys_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "HOTKEY BINDINGS", accent, |ui| {
        ui.label(egui::RichText::new("Configure custom hotkeys for quick feature toggles").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("Select a key and the feature it should toggle").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("⚠ F1 = Menu, F12 = Exit, Home = Refresh, End = Save").size(8.0).color(theme::ACCENT_WARNING));
    });
    
    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "HOTKEY SLOTS", accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Key").size(9.0).color(theme::TEXT_MUTED));
            ui.add_space(50.0);
            ui.label(egui::RichText::new("Feature").size(9.0).color(theme::TEXT_MUTED));
        });
        ui.add_space(6.0);
        
        for i in 0..10 {
            widgets::hotkey_slot(ui, i, &mut config.hotkey_bindings.slots[i]);
            ui.add_space(2.0);
        }
    });
    
    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "AVAILABLE FEATURES", accent, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            
            ui.label(egui::RichText::new("VISUAL:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("ESP, Tags, Tracers, Health, Armour, Chams, Team, Dead, Bots").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("AIM:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Aim Assist, Triggerbot, Camera Aim, Auto Reload").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("MOVEMENT:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Fly, Noclip, Spinbot, Anti-Sit, Void Hide").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("HITBOX:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Hitbox Mod, Show Visual").size(10.0).color(theme::TEXT_MUTED));
        });
    });
    
    ui.add_space(4.0);
    
    widgets::double_border_frame(ui, "HOTKEY PANEL", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.interface.show_hotkey_hints, "Show Panel", None);
    
        if config.interface.show_hotkey_hints {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Drag panel to move, or use presets:").size(9.0).color(theme::TEXT_MUTED));
            ui.horizontal(|ui| {
                if ui.add(
                    egui::Button::new(egui::RichText::new("↖ TL").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Top-Left").clicked() {
                    config.interface.hotkey_pos_x = 10.0;
                    config.interface.hotkey_pos_y = 10.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↗ TR").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Top-Right").clicked() {
                    config.interface.hotkey_pos_x = 1800.0;
                    config.interface.hotkey_pos_y = 10.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↙ BL").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Bottom-Left").clicked() {
                    config.interface.hotkey_pos_x = 10.0;
                    config.interface.hotkey_pos_y = 900.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↘ BR").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Bottom-Right").clicked() {
                    config.interface.hotkey_pos_x = 1800.0;
                    config.interface.hotkey_pos_y = 900.0;
                }
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("X:").size(9.0).color(theme::TEXT_MUTED));
                ui.add(egui::DragValue::new(&mut config.interface.hotkey_pos_x)
                    .speed(1.0)
                    .range(0.0..=3000.0));
                ui.label(egui::RichText::new("Y:").size(9.0).color(theme::TEXT_MUTED));
                ui.add(egui::DragValue::new(&mut config.interface.hotkey_pos_y)
                    .speed(1.0)
                    .range(0.0..=2000.0));
            });
        }
    });
}

fn render_misc_tab(ui: &mut egui::Ui, config: &mut Config, cache: &Arc<Cache>) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "ANTI-AFK", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.anti_afk.enabled, "Anti-AFK", None);
        ui.label(egui::RichText::new("Prevents idle kick").size(9.0).color(theme::TEXT_MUTED));
        if config.anti_afk.enabled {
            let mut interval_f32 = config.anti_afk.interval_secs as f32;
            widgets::styled_slider(ui, "Interval", &mut interval_f32, 30.0..=180.0, "s");
            config.anti_afk.interval_secs = interval_f32 as u32;
        }
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "GAME DATA", accent, |ui| {
        let cache_count = cache.count();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Players Cached").size(10.0).color(theme::TEXT_PRIMARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let color = if cache_count > 0 { theme::ACCENT_SUCCESS } else { theme::TEXT_MUTED };
            ui.label(egui::RichText::new(format!("{}", cache_count)).size(10.0).color(color).strong());
        });
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(egui::RichText::new("↻ Reload").size(9.0).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0)
                .min_size(egui::vec2(80.0, 18.0))
        ).on_hover_text("F9 - Refresh player cache").clicked() {
            super::app::PENDING_RELOAD.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        ui.add_space(4.0);

        if ui.add(
            egui::Button::new(egui::RichText::new("🔄 Full Sync").size(9.0).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0)
                .min_size(egui::vec2(80.0, 18.0))
        ).on_hover_text("Home - Re-read game instances").clicked() {
            super::app::PENDING_REFRESH.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    });
}

fn render_performance_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "FPS", accent, |ui| {
        let mut fps_f32 = config.performance.target_fps as f32;
        widgets::styled_slider(ui, "Target", &mut fps_f32, 10.0..=144.0, "");
        config.performance.target_fps = fps_f32 as u32;
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "IDLE", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.performance.idle_mode, "Idle Mode", None);
        if config.performance.idle_mode {
            let mut idle_f32 = config.performance.idle_fps as f32;
            widgets::styled_slider(ui, "Idle FPS", &mut idle_f32, 1.0..=30.0, "");
            config.performance.idle_fps = idle_f32 as u32;
        }
    });

    ui.add_space(4.0);
    widgets::double_border_frame(ui, "CACHE", accent, |ui| {
        let mut cache_f32 = config.performance.cache_update_ms as f32;
        widgets::styled_slider(ui, "Rate", &mut cache_f32, 16.0..=200.0, "ms");
        config.performance.cache_update_ms = cache_f32 as u64;
    });
}

fn render_about_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    ui.add_space(10.0);
    
    widgets::double_border_frame(ui, "accent color", accent, |ui| {
        ui.label(egui::RichText::new("customize your theme color").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        
        ui.horizontal(|ui| {
            let preview_size = egui::vec2(30.0, 20.0);
            let (preview_rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
            ui.painter().rect_filled(preview_rect, 3.0, accent);
            ui.add_space(8.0);
            ui.label(egui::RichText::new(format!("RGB({}, {}, {})", config.interface.accent_r, config.interface.accent_g, config.interface.accent_b)).size(10.0).color(theme::TEXT_SECONDARY));
        });
        
        ui.add_space(6.0);
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("R").size(10.0).color(egui::Color32::from_rgb(255, 100, 100)));
            let mut r = config.interface.accent_r as f32;
            ui.add(egui::Slider::new(&mut r, 0.0..=255.0).show_value(false));
            config.interface.accent_r = r as u8;
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("G").size(10.0).color(egui::Color32::from_rgb(100, 255, 100)));
            let mut g = config.interface.accent_g as f32;
            ui.add(egui::Slider::new(&mut g, 0.0..=255.0).show_value(false));
            config.interface.accent_g = g as u8;
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("B").size(10.0).color(egui::Color32::from_rgb(100, 100, 255)));
            let mut b = config.interface.accent_b as f32;
            ui.add(egui::Slider::new(&mut b, 0.0..=255.0).show_value(false));
            config.interface.accent_b = b as u8;
        });
        
        ui.add_space(4.0);
        
        ui.label(egui::RichText::new("presets").size(9.0).color(theme::TEXT_MUTED));
        ui.horizontal(|ui| {
            let presets = [
                ("Blue", 100u8, 100u8, 220u8),
                ("Purple", 150, 80, 200),
                ("Pink", 200, 80, 120),
                ("Cyan", 80, 180, 200),
                ("Green", 80, 200, 120),
                ("Orange", 220, 150, 80),
            ];
            for (name, r, g, b) in presets {
                let color = egui::Color32::from_rgb(r, g, b);
                let btn_size = egui::vec2(16.0, 16.0);
                let (btn_rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                ui.painter().rect_filled(btn_rect, 2.0, color);
                if response.on_hover_text(name).clicked() {
                    config.interface.accent_r = r;
                    config.interface.accent_g = g;
                    config.interface.accent_b = b;
                }
            }
        });
    });
    
    ui.add_space(10.0);
    
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(accent)
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("N").size(36.0).color(theme::BG_DARK).strong());
            });
        ui.add_space(12.0);
        ui.label(egui::RichText::new("NEXUS UNDERGROUND").size(18.0).color(theme::TEXT_PRIMARY).strong());
        ui.add_space(4.0);
        ui.label(egui::RichText::new("v2.1.0").size(12.0).color(theme::TEXT_MUTED));
    });
    
    ui.add_space(20.0);
    
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(theme::BG_LIGHT)
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(20.0, 12.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Made by").size(11.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("NexusUnderground").size(14.0).color(theme::ACCENT_PRIMARY).strong());
            });
    });
    
    ui.add_space(20.0);
    
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("“My crime is that of curiosity”").size(10.0).color(theme::TEXT_MUTED).italics());
    });
    
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(88, 101, 242)) // Discord blurple
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                if ui.add(
                    egui::Label::new(egui::RichText::new("Join Discord").size(11.0).color(egui::Color32::WHITE).strong())
                        .sense(egui::Sense::click())
                 ).on_hover_text("https://discord.gg/PUR8aU6YdY").clicked() {
                    let _ = open::that("https://discord.gg/PUR8aU6YdY");
                }
            });
    });
    
    ui.add_space(12.0);
    widgets::section_header(ui, "QUICK TIPS");
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("•").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[F1]").size(9.0).color(egui::Color32::from_rgb(100, 150, 255)).strong());
        ui.label(egui::RichText::new("toggle menu").size(9.0).color(theme::TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("•").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[END]").size(9.0).color(egui::Color32::from_rgb(100, 220, 100)).strong());
        ui.label(egui::RichText::new("save config").size(9.0).color(theme::TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("•").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[HOME]").size(9.0).color(egui::Color32::from_rgb(255, 180, 50)).strong());
        ui.label(egui::RichText::new("refresh game data").size(9.0).color(theme::TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("•").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[F12]").size(9.0).color(egui::Color32::from_rgb(255, 80, 80)).strong());
        ui.label(egui::RichText::new("exit").size(9.0).color(theme::TEXT_MUTED));
    });
}
