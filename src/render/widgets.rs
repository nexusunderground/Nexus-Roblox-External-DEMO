use eframe::egui;

use super::theme;
use crate::config::{BindableFeature, HotkeyKey, HotkeySlot};

pub fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .color(theme::TEXT_HEADER)
            .strong(),
    );
    ui.add_space(3.0);
}

pub fn double_border_frame(
    ui: &mut egui::Ui,
    title: &str,
    accent: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(theme::BG_FRAME)
        .stroke(egui::Stroke::new(1.0, theme::BORDER_FRAME))
        .rounding(3.0)
        .inner_margin(egui::Margin::same(2.0))
        .show(ui, |ui| {
            egui::Frame::none()
                .fill(theme::BG_FRAME_INNER)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_FRAME_INNER))
                .rounding(2.0)
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(title.to_uppercase())
                            .size(11.0)
                            .color(theme::TEXT_HEADER)
                            .strong(),
                    );
                    
                    let bar_width = ui.available_width().min(60.0);
                    let bar_size = egui::vec2(bar_width, 2.0);
                    let (bar_rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
                    ui.painter().rect_filled(bar_rect, 1.0, accent);
                    
                    ui.add_space(6.0);
                    
                    add_contents(ui);
                });
        });
}

pub fn styled_checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str, hotkey: Option<&str>) {
    ui.horizontal(|ui| {
        let size = egui::vec2(12.0, 12.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if response.clicked() {
            *value = !*value;
        }

        if ui.is_rect_visible(rect) {
            if *value {
                let glow_rect = rect.expand(2.0);
                ui.painter().rect_filled(glow_rect, 3.0, egui::Color32::from_rgba_unmultiplied(100, 100, 220, 40));
                ui.painter().rect_filled(rect, 2.0, theme::ACCENT_PRIMARY);
                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme::ACCENT_SECONDARY));
            } else {
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.0, theme::BORDER_DEFAULT),
                );
            }
        }

        ui.add_space(6.0);
        
        let label_color = if *value { 
            theme::ACCENT_SECONDARY 
        } else { 
            theme::TEXT_SECONDARY 
        };
        ui.label(
            egui::RichText::new(label)
                .size(11.0)
                .color(label_color),
        );

        if let Some(key) = hotkey {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(key)
                        .size(9.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        }
    });

    ui.add_space(2.0);
}

pub fn styled_toggle(ui: &mut egui::Ui, value: &mut bool, label: &str, hotkey: Option<&str>) {
    styled_checkbox(ui, value, label, hotkey);
}

pub fn styled_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label.to_lowercase()).size(11.0).color(theme::TEXT_SECONDARY));
    });

    ui.add_space(2.0);
    
    let desired_size = egui::vec2(ui.available_width(), 14.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = egui::lerp(range.clone(), t);
        }
    }
    
    if ui.is_rect_visible(rect) {
        let min = *range.start();
        let max = *range.end();
        let t = (*value - min) / (max - min);
        
        // Background track
        ui.painter().rect_filled(rect, 2.0, theme::BG_LIGHT);
        ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
        
        // Filled portion
        let fill_width = rect.width() * t;
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, rect.height()));
            ui.painter().rect_filled(fill_rect, 2.0, theme::ACCENT_PRIMARY);
        }
    }
    
    let text = if suffix.is_empty() {
        format!("{:.1}", *value)
    } else if *value < 1.0 {
        format!("{:.2}{}", *value, suffix)
    } else {
        format!("{:.0} {}", *value, suffix)
    };
    ui.label(egui::RichText::new(text).size(10.0).color(theme::TEXT_MUTED));
    
    ui.add_space(4.0);
}

pub fn editable_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
    id: &str,
) {
    let min = *range.start();
    let max = *range.end();
    
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label.to_lowercase()).size(11.0).color(theme::TEXT_SECONDARY));
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let edit_id = ui.id().with(id).with("edit_mode");
            let mut editing = ui.ctx().data_mut(|d| d.get_temp::<bool>(edit_id).unwrap_or(false));
            
            if editing {
                let input_id = ui.id().with(id).with("input");
                let just_started_id = ui.id().with(id).with("just_started");
                let just_started = ui.ctx().data_mut(|d| d.get_temp::<bool>(just_started_id).unwrap_or(true));
                
                let mut text = ui.ctx().data_mut(|d| {
                    d.get_temp::<String>(input_id).unwrap_or_else(|| format!("{:.1}", *value))
                });
                
                let response = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .desired_width(60.0)
                        .font(egui::TextStyle::Small)
                );
                
                if just_started {
                    response.request_focus();
                    ui.ctx().data_mut(|d| d.insert_temp(just_started_id, false));
                }
                
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                
                if enter {
                    if let Ok(parsed) = text.parse::<f32>() {
                        *value = parsed.clamp(min, max);
                    }
                    editing = false;
                    ui.ctx().data_mut(|d| d.insert_temp(just_started_id, true));
                } else if escape {
                    editing = false;
                    ui.ctx().data_mut(|d| d.insert_temp(just_started_id, true));
                }
                
                ui.ctx().data_mut(|d| {
                    d.insert_temp(input_id, text);
                    d.insert_temp(edit_id, editing);
                });
                
                if ui.add(
                    egui::Button::new(egui::RichText::new("OK").size(9.0).color(theme::ACCENT_SUCCESS))
                        .fill(theme::BG_DARK)
                        .stroke(egui::Stroke::new(1.0, theme::ACCENT_SUCCESS))
                        .min_size(egui::vec2(24.0, 16.0))
                ).on_hover_text("Apply value").clicked() {
                    let text = ui.ctx().data_mut(|d| d.get_temp::<String>(input_id).unwrap_or_default());
                    if let Ok(parsed) = text.parse::<f32>() {
                        *value = parsed.clamp(min, max);
                    }
                    ui.ctx().data_mut(|d| d.insert_temp(edit_id, false));
                }
            } else {
                let display = if suffix.is_empty() {
                    format!("{:.1}", *value)
                } else {
                    format!("{:.1}{}", *value, suffix)
                };
                
                if ui.add(
                    egui::Button::new(egui::RichText::new(&display).size(10.0).color(theme::TEXT_PRIMARY))
                        .fill(theme::BG_DARK)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                        .min_size(egui::vec2(50.0, 16.0))
                ).on_hover_text("Click to type value").clicked() {
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(edit_id, true);
                        d.insert_temp(ui.id().with(id).with("input"), format!("{:.1}", *value));
                    });
                }
            }
        });
    });

    ui.add_space(2.0);
    
    let desired_size = egui::vec2(ui.available_width(), 14.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = egui::lerp(range.clone(), t);
        }
    }
    
    if ui.is_rect_visible(rect) {
        let t = (*value - min) / (max - min);
        
        // Background track
        ui.painter().rect_filled(rect, 2.0, theme::BG_LIGHT);
        ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
        
        // Filled portion
        let fill_width = rect.width() * t;
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, rect.height()));
            ui.painter().rect_filled(fill_rect, 2.0, theme::ACCENT_PRIMARY);
        }
    }
    
    ui.add_space(4.0);
}

pub fn bone_selector(ui: &mut egui::Ui, label: &str, value: &mut String, id: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label.to_lowercase()).size(10.0).color(theme::TEXT_SECONDARY));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let display_text = format!("{}", value.as_str());
            egui::ComboBox::from_id_source(id)
                .selected_text(egui::RichText::new(display_text).color(theme::ACCENT_PRIMARY))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, "Head".to_string(), "Head");
                    ui.selectable_value(value, "UpperTorso".to_string(), "Torso");
                    ui.selectable_value(value, "HumanoidRootPart".to_string(), "Center");
                });
        });
    });
}

pub fn activation_mode_selector(ui: &mut egui::Ui, label: &str, value: &mut u8, id: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label.to_lowercase()).size(10.0).color(theme::TEXT_SECONDARY));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let display = match *value {
                0 => "Hold",
                1 => "Toggle",
                2 => "Always",
                _ => "Hold",
            };
            let display_text = format!("{}", display);
            egui::ComboBox::from_id_source(id)
                .selected_text(egui::RichText::new(display_text).color(theme::ACCENT_PRIMARY))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, 0, "Hold");
                    ui.selectable_value(value, 1, "Toggle");
                    ui.selectable_value(value, 2, "Always");
                });
        });
    });
}

pub fn fly_mode_selector(ui: &mut egui::Ui, label: &str, value: &mut u8, id: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label.to_lowercase()).size(10.0).color(theme::TEXT_SECONDARY));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if *value > 1 {
                *value = 1;
            }
            let display = match *value {
                0 => "Velocity",
                _ => "Stable",
            };
            let display_text = format!("{} v", display);
            egui::ComboBox::from_id_source(id)
                .selected_text(egui::RichText::new(display_text).color(theme::ACCENT_PRIMARY))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, 0, "Velocity");
                    ui.selectable_value(value, 1, "Stable");
                });
        });
    });
}

pub fn write_intensity_selector(ui: &mut egui::Ui, value: &mut u8) {
    ui.horizontal(|ui| {
        let labels = [(1u8, "Low (60/s)"), (2u8, "Med (120/s)"), (3u8, "High (180/s)")];
        for (v, label) in labels {
            let selected = *value == v;
            let text_color = if selected { theme::ACCENT_PRIMARY } else { theme::TEXT_MUTED };
            let bg = if selected { theme::BG_LIGHT } else { theme::BG_DARK };
            if ui.add(
                egui::Button::new(egui::RichText::new(label).size(9.0).color(text_color))
                    .fill(bg)
                    .stroke(if selected { egui::Stroke::new(1.0, theme::ACCENT_PRIMARY) } else { egui::Stroke::NONE })
                    .rounding(3.0)
            ).clicked() {
                *value = v;
            }
        }
    });
}

pub fn aim_section_header(
    ui: &mut egui::Ui,
    title: &str,
    description: Option<&str>,
    enabled: &mut bool,
    section_id: u8,
    expanded_section: &mut u8,
    hotkey: Option<&str>,
) -> bool {
    let is_expanded = *expanded_section == section_id;
    
    egui::Frame::none()
        .fill(theme::BG_FRAME)
        .stroke(egui::Stroke::new(1.0, if *enabled { theme::BORDER_ACTIVE } else { theme::BORDER_FRAME }))
        .rounding(3.0)
        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let checkbox_size = egui::vec2(10.0, 10.0);
                let (checkbox_rect, checkbox_response) = ui.allocate_exact_size(checkbox_size, egui::Sense::click());
                
                if checkbox_response.clicked() {
                    *enabled = !*enabled;
                }
                
                if ui.is_rect_visible(checkbox_rect) {
                    ui.painter().rect_stroke(checkbox_rect, 1.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
                    if *enabled {
                        let inner = checkbox_rect.shrink(2.0);
                        ui.painter().rect_filled(inner, 0.0, theme::ACCENT_PRIMARY);
                    }
                }
                
                ui.add_space(6.0);
                
                let title_response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(title.to_lowercase())
                            .size(11.0)
                            .color(if *enabled { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY })
                    ).sense(egui::Sense::click())
                );
                
                if title_response.clicked() {
                    if is_expanded {
                        *expanded_section = 0;
                    } else {
                        *expanded_section = section_id;
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let arrow = if is_expanded { "^" } else { "v" };
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new(arrow)
                                .size(10.0)
                                .color(theme::ACCENT_PRIMARY)
                        )
                        .fill(theme::BG_DARK)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                        .min_size(egui::vec2(18.0, 18.0))
                    ).on_hover_text(if is_expanded { "Collapse" } else { "Expand options" }).clicked() {
                        if is_expanded {
                            *expanded_section = 0;
                        } else {
                            *expanded_section = section_id;
                        }
                    }
                    
                    if let Some(key) = hotkey {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(key).size(9.0).color(theme::ACCENT_PRIMARY).strong());
                    }
                });
            });
            
            if !is_expanded {
                if let Some(desc) = description {
                    ui.label(egui::RichText::new(desc).size(8.0).color(theme::TEXT_MUTED));
                }
            }
        });
    
    ui.add_space(2.0);
    is_expanded
}

pub fn hotkey_slot(ui: &mut egui::Ui, slot_index: usize, slot: &mut HotkeySlot) {
    let combo_style = ui.style_mut();
    combo_style.visuals.widgets.inactive.weak_bg_fill = theme::BG_DARK;
    combo_style.visuals.widgets.hovered.weak_bg_fill = theme::BG_MEDIUM;
    combo_style.visuals.widgets.active.weak_bg_fill = theme::BG_MEDIUM;
    combo_style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    combo_style.visuals.window_fill = theme::BG_DARK;
    
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{}.", slot_index + 1))
                .size(10.0)
                .color(theme::TEXT_MUTED)
        );
        
        let key_id = format!("hotkey_key_{}", slot_index);
        egui::ComboBox::from_id_source(&key_id)
            .selected_text(egui::RichText::new(slot.key.display_name()).size(10.0).color(theme::ACCENT_PRIMARY))
            .width(55.0)
            .show_ui(ui, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = theme::BG_DARK;
                egui::Frame::none()
                    .fill(theme::BG_DARK)
                    .show(ui, |ui| {
                        for key in HotkeyKey::all_keys() {
                            ui.selectable_value(&mut slot.key, *key, key.display_name());
                        }
                    });
            });
        
        ui.label(egui::RichText::new("→").size(10.0).color(theme::TEXT_MUTED));
        
        let feature_id = format!("hotkey_feature_{}", slot_index);
        let display_text = if slot.feature == BindableFeature::None {
            "Select Feature...".to_string()
        } else {
            slot.feature.display_name().to_string()
        };
        
        egui::ComboBox::from_id_source(&feature_id)
            .selected_text(egui::RichText::new(&display_text).size(10.0).color(
                if slot.feature == BindableFeature::None { theme::TEXT_MUTED } else { theme::TEXT_PRIMARY }
            ))
            .width(130.0)
            .show_ui(ui, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = theme::BG_DARK;
                egui::Frame::none()
                    .fill(theme::BG_DARK)
                    .show(ui, |ui| {
                        let mut current_category = "";
                        for feature in BindableFeature::all_features() {
                            let category = feature.category();
                            if !category.is_empty() && category != current_category {
                                if !current_category.is_empty() {
                                    ui.add_space(4.0);
                                }
                                ui.label(
                                    egui::RichText::new(category)
                                        .size(9.0)
                                        .color(theme::ACCENT_INFO)
                                        .strong()
                                );
                                current_category = category;
                            }
                            ui.selectable_value(&mut slot.feature, *feature, feature.display_name());
                        }
                    });
            });
    });
}
