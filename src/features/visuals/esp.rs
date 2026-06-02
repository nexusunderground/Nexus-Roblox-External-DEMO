//! ESP (Extra Sensory Perception) rendering.
//!
//! Renders boxes, names, health bars, and other visual aids on enemy players.

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;

use crate::config::Config;
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::base_part;
use crate::sdk::{Instance, VisualEngine};
use crate::utils::cache::{Cache, BodyPart};
use crate::utils::math::{Vector2, Vector3};

// Per-frame render-thread cache: display name â†’ name pixel width for role-tag
// X-offset calculation.  Avoids repeated `layout_no_wrap` calls for names that
// do not change between frames (which is almost always the case in Roblox).
//
// Keyed by `(display_name, font_size_as_u32_bits)` so a font-size config change
// invalidates the old width automatically.  Cache grows to at most ~max_players
// entries and is intentionally never cleared (names are stable per session).
thread_local! {
    static ROLE_TAG_WIDTH_CACHE: RefCell<ahash::AHashMap<(String, u32), f32>> =
        RefCell::new(ahash::AHashMap::new());
}

/// Helper: return the pixel width of `text` laid out with `font`, using the
/// thread-local cache to avoid re-layout on unchanged text.
#[inline]
fn cached_text_width(
    painter: &egui::Painter,
    text: &str,
    font: &egui::FontId,
    color: egui::Color32,
) -> f32 {
    ROLE_TAG_WIDTH_CACHE.with(|cache| {
        let key = (text.to_owned(), font.size.to_bits());
        // Immutable borrow to check cache â€” dropped before mutable borrow on miss.
        if let Some(&w) = cache.borrow().get(&key) {
            return w;
        }
        let gly = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
        let w = gly.size().x;
        cache.borrow_mut().insert(key, w);
        w
    })
}
/// ESP rendering system.
pub struct Esp;

impl Esp {
    /// Render FOV circle.
    pub fn render_fov_circle(ctx: &egui::Context, config: &Config, visengine: &Arc<VisualEngine>) {
        crate::perf_scope!("esp_render_fov_circle");
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();
        let center = egui::pos2(
            window_offset.x + dimensions.x / 2.0,
            window_offset.y + dimensions.y / 2.0,
        );

        // Aimbot FOV circle (indigo)
        if config.aimbot.enabled && config.aimbot.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_aimbot"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.aimbot.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(99, 102, 241, 100)),
                    );
                });
        }

        // Camera aim FOV circle (cyan/green)
        if config.camera_aim.enabled && config.camera_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_camera"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.camera_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(34, 211, 153, 100)),
                    );
                });
        }

        // Universal camera aim FOV circle (purple)
        // (removed)

        // Viewport aim FOV circle (orange)
        if config.viewport_aim.enabled && config.viewport_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_viewport"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.viewport_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(251, 146, 60, 120)),
                    );
                });
        }

        // Silent aim (MouseService) FOV circle (red/pink)
        if config.silent_aim.enabled && config.silent_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_silent"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.silent_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(244, 63, 94, 120)),
                    );
                });
        }

        // Mouse-move aim FOV circle (yellow)
        if config.mouse_aim.enabled && config.mouse_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_mouse_aim"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.mouse_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(250, 204, 21, 120)),
                    );
                });
        }

    }

    /// Render ESP using pre-computed cached data (high-performance path).
    /// 
    /// Metadata (health, teams, visibility, etc.) comes from the background ESP data thread.
    /// Screen positions are re-projected from world-space each frame using the CURRENT
    /// view matrix to eliminate lag when the camera moves.
    pub fn render_cached(
        ctx: &egui::Context,
        esp_cache: &Arc<super::esp_cache::EspRenderCache>,
        config: &Config,
        visengine: &Arc<VisualEngine>,
    ) {
        crate::perf_scope!("esp_render_cached");
        let render_data = esp_cache.get_render_data();
        if render_data.is_empty() {
            return;
        }

        // Use FRESH view matrix from visengine for frame-accurate screen projection.
        // This is the key fix: the background thread caches entity metadata, but screen
        // positions are always computed with the current camera to prevent ESP "lag".
        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let box_enabled = config.visuals.box_esp;
        let show_name    = config.visuals.name_tags;
        let show_dist    = config.visuals.show_distance_label;
        let show_weapon  = config.visuals.show_equipped_weapon;
        let any_label    = show_name || show_dist || show_weapon;
        let tracers_enabled = config.visuals.tracers;
        let distance_colors = config.visuals.distance_colors;
        let target_highlight = config.visuals.target_highlight;
        let team_check = config.visuals.team_check;
        let wall_check = config.visuals.wall_check;

        if !box_enabled && !any_label && !tracers_enabled {
            return;
        }

        // Single Area for all ESP rendering - minimal overhead
        egui::Area::new(egui::Id::new("esp_overlay_cached"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(egui::Rect::EVERYTHING);
                let painter = ui.painter();


                for data in render_data.iter() {
                    // Hide teammates entirely if team_hide_visuals is on
                    if data.is_teammate && team_check && config.visuals.team_hide_visuals {
                        continue;
                    }

                    // â”€â”€ Determine ESP color (independent of projection) â”€â”€
                    let occluded = wall_check && !data.is_visible;
                    let esp_color = if !data.role_tag.is_empty() {
                        // MM2 role-based coloring with wall check dimming
                        let (r, g, b) = match data.role_tag.as_ref() {
                            "Murderer" => (255u8, 60u8, 60u8),
                            "Sheriff" => (255u8, 215u8, 0u8),
                            _ => if data.is_teammate && team_check {
                                (59u8, 130u8, 246u8)
                            } else {
                                (140u8, 200u8, 140u8)
                            },
                        };
                        if occluded {
                            egui::Color32::from_rgb(r / 2, g / 2, b / 2)
                        } else {
                            egui::Color32::from_rgb(r, g, b)
                        }
                    } else if data.is_teammate && team_check {
                        if occluded {
                            egui::Color32::from_rgb(30, 65, 123)
                        } else {
                            egui::Color32::from_rgb(59, 130, 246)
                        }
                    } else if data.is_aim_target && target_highlight {
                        egui::Color32::from_rgb(255, 0, 255)
                    } else if occluded {
                        egui::Color32::from_rgb(255, 80, 80)
                    } else if distance_colors {
                        Self::get_distance_color(data.distance)
                    } else {
                        egui::Color32::from_rgb(
                            (config.visuals.box_color[0] * 255.0) as u8,
                            (config.visuals.box_color[1] * 255.0) as u8,
                            (config.visuals.box_color[2] * 255.0) as u8,
                        )
                    };

                    // â”€â”€ Tracers â”€â”€
                    if tracers_enabled {
                        if let Some(target) = visengine.world_to_screen_wide(data.world_pos, dimensions, &view_matrix) {
                            let sb_x = window_offset.x + dimensions.x / 2.0;
                            let sb_y = window_offset.y + dimensions.y;
                            let tp_x = target.x + window_offset.x;
                            let tp_y = target.y + window_offset.y;
                                painter.line_segment(
                                    [egui::pos2(sb_x, sb_y), egui::pos2(tp_x, tp_y)],
                                    egui::Stroke::new(1.0, esp_color),
                                );
                        }
                    }

                    // â”€â”€ Projection â”€â”€
                    let bottom_screen = match visengine.world_to_screen(data.world_bottom, dimensions, &view_matrix) {
                        Some(v) => Vector2::new(v.x + window_offset.x, v.y + window_offset.y),
                        None => continue,
                    };
                    let top_screen = match visengine.world_to_screen(data.world_top, dimensions, &view_matrix) {
                        Some(v) => Vector2::new(v.x + window_offset.x, v.y + window_offset.y),
                        None => continue,
                    };

                    // Skip targets whose bounding box is entirely off-screen.
                    // This prevents drawing ESP elements for targets outside the
                    // visible area when the Roblox window is small/resized.
                    let screen_right = window_offset.x + dimensions.x;
                    let screen_bottom = window_offset.y + dimensions.y;
                    let min_x = bottom_screen.x.min(top_screen.x);
                    let max_x = bottom_screen.x.max(top_screen.x);
                    let min_y = bottom_screen.y.min(top_screen.y);
                    let max_y = bottom_screen.y.max(top_screen.y);
                    if max_x < window_offset.x || min_x > screen_right || max_y < window_offset.y || min_y > screen_bottom {
                        continue;
                    }

                    let base_height = (bottom_screen.y - top_screen.y).abs();
                    let center_x = (top_screen.x + bottom_screen.x) / 2.0;
                    let base_top = top_screen.y.min(bottom_screen.y);
                    let base_bottom = top_screen.y.max(bottom_screen.y);

                    let box_width = base_height * 0.65;
                    let v_padding_top = base_height * 0.06;
                    let v_padding_bottom = base_height * 0.08;

                    let box_left = center_x - box_width / 2.0;
                    let box_right = center_x + box_width / 2.0;
                    let box_top = base_top - v_padding_top;
                    let box_bottom = base_bottom + v_padding_bottom;

                    if (box_bottom - box_top) < 5.0 {
                        continue;
                    }

                    // â”€â”€ Box â”€â”€
                    if box_enabled {


                        if config.visuals.box_style == 2 {
                            // 3D box
                            let corners_3d = data.box_3d_corners_world.as_ref().and_then(|world_corners| {
                                let mut screen_pts = [Vector2::ZERO; 8];
                                for (i, wc) in world_corners.iter().enumerate() {
                                    match visengine.world_to_screen(*wc, dimensions, &view_matrix) {
                                        Some(sp) => screen_pts[i] = Vector2::new(sp.x + window_offset.x, sp.y + window_offset.y),
                                        None => return None,
                                    }
                                }
                                Some(screen_pts)
                            });
                            if let Some(corners) = corners_3d {
                                if config.visuals.box_fill {
                                    let fill_alpha = config.visuals.box_fill_opacity;
                                    let fc = [
                                        config.visuals.box_fill_color[0],
                                        config.visuals.box_fill_color[1],
                                        config.visuals.box_fill_color[2],
                                        fill_alpha,
                                    ];
                                        painter.rect_filled(
                                            egui::Rect::from_min_max(egui::pos2(box_left, box_top), egui::pos2(box_right, box_bottom)),
                                            0.0,
                                            egui::Color32::from_rgba_unmultiplied(
                                                (fc[0] * 255.0) as u8, (fc[1] * 255.0) as u8,
                                                (fc[2] * 255.0) as u8, (fc[3] * 255.0) as u8,
                                            ),
                                        );
                                }
                                const EDGES: [(usize, usize); 12] = [
                                    (0,1),(1,2),(2,3),(3,0),
                                    (4,5),(5,6),(6,7),(7,4),
                                    (0,4),(1,5),(2,6),(3,7),
                                ];
                                    Self::draw_3d_box(painter, &corners, esp_color);
                            }
                        } else {
                            // Box fill
                            if config.visuals.box_fill {
                                let fill_alpha = config.visuals.box_fill_opacity;
                                let fc = [
                                    config.visuals.box_fill_color[0],
                                    config.visuals.box_fill_color[1],
                                    config.visuals.box_fill_color[2],
                                    fill_alpha,
                                ];
                                    painter.rect_filled(
                                        egui::Rect::from_min_max(egui::pos2(box_left, box_top), egui::pos2(box_right, box_bottom)),
                                        0.0,
                                        egui::Color32::from_rgba_unmultiplied(
                                            (fc[0] * 255.0) as u8, (fc[1] * 255.0) as u8,
                                            (fc[2] * 255.0) as u8, (fc[3] * 255.0) as u8,
                                        ),
                                    );
                            }

                            // Box outline
                            if config.visuals.box_style == 1 {
                                    Self::draw_corner_box(painter, box_left, box_top, box_right, box_bottom, esp_color);
                            } else {
                                    painter.rect_stroke(
                                        egui::Rect::from_min_max(egui::pos2(box_left, box_top), egui::pos2(box_right, box_bottom)),
                                        0.0,
                                        egui::Stroke::new(1.0, esp_color),
                                    );
                            }
                        }
                    }

                    // â”€â”€ Health bar â”€â”€
                    if config.visuals.health_bars {
                        let bar_w = 4.0;
                        let bx = box_left - bar_w - 2.0;
                        let bh = box_bottom - box_top;
                        let fill_h = bh * data.health_percent;
                        let fill_top = box_bottom - fill_h;
                        let hc = if data.health_percent > 0.6 {
                            [0.0, 1.0, 0.0, 1.0]
                        } else if data.health_percent > 0.3 {
                            [1.0, 1.0, 0.0, 1.0]
                        } else {
                            [1.0, 0.0, 0.0, 1.0]
                        };
                            Self::draw_health_bar(painter, box_left, box_top, box_bottom, data.health_percent);
                    }

                    // â”€â”€ Armor bar â”€â”€
                    if config.visuals.armor_bars && data.has_armor {
                        let armor_offset = if config.visuals.health_bars { -12.0 } else { -6.0 };
                        let bx = box_left + armor_offset;
                        let bar_w = 4.0;
                        let bh = box_bottom - box_top;
                        let fill_h = bh * data.armor_percent;
                        let fill_top = box_bottom - fill_h;
                        let ac = [59.0/255.0, 130.0/255.0, 246.0/255.0, 1.0];
                            Self::draw_armor_bar(painter, box_left + armor_offset, box_top, box_bottom, data.armor_percent);
                    }

                    // â”€â”€ Labels: name / distance / weapon â€” each independently toggled â”€â”€
                    if any_label {
                        let weapon_active = show_weapon && !data.equipped_weapon.is_empty();

                        // Config-driven font sizes (created per-frame; FontId is a cheap struct)
                        let name_font   = egui::FontId::proportional(config.visuals.esp_name_size);
                        let dist_font   = egui::FontId::proportional(config.visuals.esp_dist_size);
                        let weapon_font = egui::FontId::proportional(config.visuals.esp_weapon_size);
                        static ROLE_FONT: std::sync::LazyLock<egui::FontId> =
                            std::sync::LazyLock::new(|| egui::FontId::proportional(12.0));

                        // Config-driven label colours
                        let to_c32 = |rgba: &[f32; 4]| egui::Color32::from_rgba_unmultiplied(
                            (rgba[0] * 255.0) as u8, (rgba[1] * 255.0) as u8, (rgba[2] * 255.0) as u8, (rgba[3] * 255.0) as u8);
                        let name_col   = to_c32(&config.visuals.esp_name_color);
                        let dist_col   = to_c32(&config.visuals.esp_dist_color);
                        let weapon_col = to_c32(&config.visuals.esp_weapon_color);

                        // Y-position stacking: only active labels consume a stack slot.
                        const STEP: f32 = 14.0;
                        let mut ai = 0u32;
                        let mut bi = 0u32;

                        let name_y: Option<f32> = if show_name {
                            Some(if config.visuals.esp_name_pos == 0 {
                                let i = ai; ai += 1; box_top - STEP * (i as f32 + 1.0)
                            } else {
                                let i = bi; bi += 1; box_bottom + 2.0 + STEP * i as f32
                            })
                        } else { None };
                        let dist_y: Option<f32> = if show_dist {
                            Some(if config.visuals.esp_dist_pos == 0 {
                                let i = ai; ai += 1; box_top - STEP * (i as f32 + 1.0)
                            } else {
                                let i = bi; bi += 1; box_bottom + 2.0 + STEP * i as f32
                            })
                        } else { None };
                        let weapon_y: Option<f32> = if weapon_active {
                            Some(if config.visuals.esp_weapon_pos == 0 {
                                let i = ai; ai += 1; box_top - STEP * (i as f32 + 1.0)
                            } else {
                                let i = bi; bi += 1; box_bottom + 2.0 + STEP * i as f32
                            })
                        } else { None };
                        let _ = (ai, bi);

                        let display_name = name_y.as_ref().map(|_|
                            data.name.clone());

                            if let (Some(ny), Some(ref dname)) = (name_y, &display_name) {
                                painter.text(egui::pos2(box_left, ny), egui::Align2::LEFT_TOP,
                                    dname.as_ref(), name_font.clone(), name_col);
                                if !data.role_tag.is_empty() {
                                    let (rc, rl) = match data.role_tag.as_ref() {
                                        "Murderer" => (egui::Color32::from_rgb(255, 60, 60), "[MURDERER]"),
                                        "Sheriff"  => (egui::Color32::from_rgb(255, 215, 0),  "[SHERIFF]"),
                                        _          => (egui::Color32::from_rgb(140, 200, 140), "[INNOCENT]"),
                                    };
                                    let name_w = cached_text_width(&painter, dname.as_ref(), &name_font, name_col);
                                    painter.text(egui::pos2(box_left + name_w + 4.0, ny + 1.0),
                                        egui::Align2::LEFT_TOP, rl, ROLE_FONT.clone(), rc);
                                }
                            }
                            if let Some(dy) = dist_y {
                                painter.text(egui::pos2(box_left, dy), egui::Align2::LEFT_TOP,
                                    format!("{}m", data.distance as u32), dist_font, dist_col);
                            }
                            if let Some(wy) = weapon_y {
                                painter.text(egui::pos2(box_left, wy), egui::Align2::LEFT_TOP,
                                    data.equipped_weapon.as_ref(), weapon_font, weapon_col);
                            }
                    }

                    // â”€â”€ Game-specific entity name (slime enemies/loot) â”€â”€
                    // Shown when game ESP is active but generic name_tags is off.
                    // Falls back gracefully when any_label is already true.
                    if !any_label && data.is_game_specific {
                        let name_font = egui::FontId::proportional(11.0);
                        let ny = top_screen.y - 2.0;
                        let nx = center_x;
                            painter.text(
                                egui::pos2(nx, ny),
                                egui::Align2::CENTER_BOTTOM,
                                data.name.as_ref(),
                                name_font,
                                esp_color,
                            );
                    }
                }


            });
    }

    fn get_distance_color(distance: f32) -> egui::Color32 {
        if distance < 30.0 {
            egui::Color32::from_rgb(0, 255, 0)
        } else if distance < 80.0 {
            egui::Color32::from_rgb(0, 255, 128)
        } else if distance < 150.0 {
            egui::Color32::from_rgb(255, 255, 0)
        } else {
            egui::Color32::from_rgb(255, 100, 100)
        }
    }

    /// Draw corner-only box style (4 L-shaped corners)
    /// More performant than full box - only 8 line segments instead of 4 rect sides
    #[inline]
    fn draw_corner_box(painter: &egui::Painter, left: f32, top: f32, right: f32, bottom: f32, color: egui::Color32) {
        let width = right - left;
        let height = bottom - top;
        // Corner length is 20% of the smaller dimension, clamped for small boxes
        let corner_len = (width.min(height) * 0.20).max(4.0).min(20.0);
        let stroke = egui::Stroke::new(1.2, color);
        
        // Top-left corner
        painter.line_segment([egui::pos2(left, top), egui::pos2(left + corner_len, top)], stroke);
        painter.line_segment([egui::pos2(left, top), egui::pos2(left, top + corner_len)], stroke);
        
        // Top-right corner
        painter.line_segment([egui::pos2(right - corner_len, top), egui::pos2(right, top)], stroke);
        painter.line_segment([egui::pos2(right, top), egui::pos2(right, top + corner_len)], stroke);
        
        // Bottom-left corner
        painter.line_segment([egui::pos2(left, bottom - corner_len), egui::pos2(left, bottom)], stroke);
        painter.line_segment([egui::pos2(left, bottom), egui::pos2(left + corner_len, bottom)], stroke);
        
        // Bottom-right corner
        painter.line_segment([egui::pos2(right - corner_len, bottom), egui::pos2(right, bottom)], stroke);
        painter.line_segment([egui::pos2(right, bottom - corner_len), egui::pos2(right, bottom)], stroke);
    }

    /// Draw a 3D wireframe box from 8 projected screen-space corners.
    /// Corner layout:
    ///   Top face: 0-1-2-3  (front-left, front-right, back-right, back-left)
    ///   Bottom face: 4-5-6-7 (same order)
    #[inline]
    fn draw_3d_box(painter: &egui::Painter, corners: &[Vector2; 8], color: egui::Color32) {
        let outline = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);
        let outline_stroke = egui::Stroke::new(3.0, outline);
        let stroke = egui::Stroke::new(1.5, color);

        // 12 edges of a cuboid
        const EDGES: [(usize, usize); 12] = [
            // Top face
            (0, 1), (1, 2), (2, 3), (3, 0),
            // Bottom face
            (4, 5), (5, 6), (6, 7), (7, 4),
            // Vertical pillars
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];

        // Outline pass
        for &(a, b) in &EDGES {
            painter.line_segment(
                [egui::pos2(corners[a].x, corners[a].y), egui::pos2(corners[b].x, corners[b].y)],
                outline_stroke,
            );
        }
        // Color pass
        for &(a, b) in &EDGES {
            painter.line_segment(
                [egui::pos2(corners[a].x, corners[a].y), egui::pos2(corners[b].x, corners[b].y)],
                stroke,
            );
        }
    }

    fn draw_health_bar(painter: &egui::Painter, box_left: f32, box_top: f32, box_bottom: f32, health_percent: f32) {
        let bar_width = 4.0;
        let bar_height = box_bottom - box_top;
        let bar_x = box_left - bar_width - 2.0;

        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));

        let fill_height = bar_height * health_percent;
        let fill_top = box_bottom - fill_height;

        let health_color = if health_percent > 0.6 {
            egui::Color32::GREEN
        } else if health_percent > 0.3 {
            egui::Color32::YELLOW
        } else {
            egui::Color32::RED
        };

        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, fill_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(fill_rect, 0.0, health_color);
        painter.rect_stroke(bg_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }

    fn draw_armor_bar(painter: &egui::Painter, bar_x: f32, box_top: f32, box_bottom: f32, armor_percent: f32) {
        let bar_width = 4.0;
        let bar_height = box_bottom - box_top;

        // Background
        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));

        let fill_height = bar_height * armor_percent;
        let fill_top = box_bottom - fill_height;

        // Armor is typically displayed as blue/cyan
        let armor_color = egui::Color32::from_rgb(59, 130, 246); // Blue color

        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, fill_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(fill_rect, 0.0, armor_color);
        painter.rect_stroke(bg_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }

    /// Render hitbox visualization for expanded hitboxes.
    /// This shows a colored box around players where hitbox expansion is active.
    pub fn render_hitbox_visual(
        ctx: &egui::Context,
        cache: &Arc<Cache>,
        visengine: &Arc<VisualEngine>,
        config: &crate::config::Config,
        local_player_name: &str,
    ) {
        // Only render if hitbox expander is enabled AND visual is enabled
        if !config.hitbox.enabled || !config.hitbox.show_visual {
            return;
        }

        let snapshot = cache.get_snapshot();
        if snapshot.is_empty() {
            return;
        }

        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        // Get color from config
        let hitbox_color = egui::Color32::from_rgba_unmultiplied(
            (config.hitbox.color[0] * 255.0) as u8,
            (config.hitbox.color[1] * 255.0) as u8,
            (config.hitbox.color[2] * 255.0) as u8,
            (config.hitbox.color[3] * 255.0) as u8,
        );
        let hitbox_fill = egui::Color32::from_rgba_unmultiplied(
            (config.hitbox.color[0] * 255.0) as u8,
            (config.hitbox.color[1] * 255.0) as u8,
            (config.hitbox.color[2] * 255.0) as u8,
            ((config.hitbox.color[3] * 0.3) * 255.0) as u8, // 30% of alpha for fill
        );

        let hitbox_size_x = config.hitbox.head_scale * 2.0;
        let hitbox_size_y = config.hitbox.torso_scale * 2.0;
        let _hitbox_size_z = config.hitbox.arms_scale * 2.0; // Reserved for future 3D visualization

        let local_pos = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name))
            .and_then(|e| e.parts.get(&BodyPart::HumanoidRootPart))
            .map(|p| p.position)
            .unwrap_or(Vector3::ZERO);

        // Collect hitbox data
        let hitbox_data: Vec<_> = snapshot
            .iter()
            .filter(|entity| {
                // Skip local player unless self_enabled
                if entity.name.eq_ignore_ascii_case(local_player_name) {
                    config.hitbox.self_enabled
                } else {
                    config.hitbox.enemy_enabled
                }
            })
            .filter_map(|entity| {
                let hrp = entity.parts.get(&BodyPart::HumanoidRootPart)?;
                let pos = hrp.position;

                if (pos.x == 0.0 && pos.y == 0.0 && pos.z == 0.0)
                    || pos.x.is_nan() || pos.y.is_nan() || pos.z.is_nan()
                {
                    return None;
                }

                let distance = pos.distance_to(local_pos);
                if distance > config.visuals.max_distance {
                    return None;
                }

                let screen_center = visengine.world_to_screen(pos, dimensions, &view_matrix)?;
                let screen_center = Vector2::new(
                    screen_center.x + window_offset.x,
                    screen_center.y + window_offset.y,
                );

                let base_scale = 1200.0 / distance.max(10.0);
                let visual_width = base_scale * hitbox_size_x * 0.8;
                let visual_height = base_scale * hitbox_size_y * 0.8;

                if visual_width < 5.0 || visual_height < 5.0 {
                    return None;
                }

                Some((screen_center, visual_width, visual_height))
            })
            .collect();

        // Render
        egui::Area::new(egui::Id::new("hitbox_visual_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();

                for (screen_center, width, height) in &hitbox_data {
                    let rect = egui::Rect::from_center_size(
                        egui::pos2(screen_center.x, screen_center.y),
                        egui::vec2(*width, *height),
                    );

                    painter.rect_filled(rect, 4.0, hitbox_fill);
                    painter.rect_stroke(rect, 4.0, egui::Stroke::new(2.0, hitbox_color));
                }
            });
    }


    /// Render desync visualizer â€” a ghost marker at the frozen position.
    /// Shows where other players see you standing while desync is active.
    pub fn render_desync_visualizer(
        ctx: &egui::Context,
        visengine: &Arc<VisualEngine>,
        frozen_pos: Vector3,
    ) {
        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let screen = match visengine.world_to_screen(frozen_pos, dimensions, &view_matrix) {
            Some(s) => s,
            None => return,
        };

        let center = egui::pos2(screen.x + window_offset.x, screen.y + window_offset.y);
        let ghost_color = egui::Color32::from_rgba_unmultiplied(255, 165, 0, 180); // Orange, semi-transparent
        let ring_color = egui::Color32::from_rgba_unmultiplied(255, 165, 0, 100);

        egui::Area::new(egui::Id::new("desync_visualizer"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(egui::Rect::EVERYTHING);
                let painter = ui.painter();

                // Filled inner circle
                painter.circle_filled(center, 6.0, ghost_color);
                // Outer pulsing rings
                painter.circle_stroke(center, 14.0, egui::Stroke::new(1.5, ring_color));
                painter.circle_stroke(center, 22.0, egui::Stroke::new(1.0, ring_color));
                // Cross through center
                let cs = 10.0;
                painter.line_segment(
                    [egui::pos2(center.x - cs, center.y), egui::pos2(center.x + cs, center.y)],
                    egui::Stroke::new(1.2, ghost_color),
                );
                painter.line_segment(
                    [egui::pos2(center.x, center.y - cs), egui::pos2(center.x, center.y + cs)],
                    egui::Stroke::new(1.2, ghost_color),
                );
                // Label
                painter.text(
                    egui::pos2(center.x, center.y - 28.0),
                    egui::Align2::CENTER_BOTTOM,
                    "GHOST",
                    egui::FontId::proportional(10.0),
                    ghost_color,
                );
            });
    }

    /// Render waypoint marker â€” a pin at the saved waypoint position.
    /// Shows where the player will teleport to when using waypoint recall.
    pub fn render_waypoint_marker(
        ctx: &egui::Context,
        visengine: &Arc<VisualEngine>,
        waypoint_pos: Vector3,
    ) {
        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let screen = match visengine.world_to_screen(waypoint_pos, dimensions, &view_matrix) {
            Some(s) => s,
            None => return,
        };

        let center = egui::pos2(screen.x + window_offset.x, screen.y + window_offset.y);
        let marker_color = egui::Color32::from_rgba_unmultiplied(0, 200, 255, 200); // Cyan
        let ring_color = egui::Color32::from_rgba_unmultiplied(0, 200, 255, 100);

        egui::Area::new(egui::Id::new("waypoint_marker"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(egui::Rect::EVERYTHING);
                let painter = ui.painter();

                // Diamond shape (rotated square)
                let d = 8.0;
                let diamond = vec![
                    egui::pos2(center.x, center.y - d),
                    egui::pos2(center.x + d, center.y),
                    egui::pos2(center.x, center.y + d),
                    egui::pos2(center.x - d, center.y),
                ];
                painter.add(egui::Shape::convex_polygon(diamond, marker_color, egui::Stroke::NONE));

                // Outer rings
                painter.circle_stroke(center, 16.0, egui::Stroke::new(1.5, ring_color));
                painter.circle_stroke(center, 24.0, egui::Stroke::new(1.0, ring_color));

                // Label
                painter.text(
                    egui::pos2(center.x, center.y - 30.0),
                    egui::Align2::CENTER_BOTTOM,
                    "WAYPOINT",
                    egui::FontId::proportional(10.0),
                    marker_color,
                );
            });
    }

}
