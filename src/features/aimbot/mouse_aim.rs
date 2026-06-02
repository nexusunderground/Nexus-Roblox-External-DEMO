//! Mouse-Move Aim — Direct mouse delta aim via SendInput.

use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::Memory;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache, Entity};
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{is_teammate, VELOCITY_THRESHOLD, RingVelocityTracker};
use crate::utils::targeting::get_bone_with_fallback;

use ahash::AHashSet;
use std::collections::HashMap;

const DEAD_ZONE_PX: f32 = 0.8;
const MAX_MOUSE_DELTA: f32 = 80.0;
const MAX_FRAME_CORRECTION: f32 = 0.35;
const LOCK_BREAK_DISTANCE: f32 = 500.0;
const TARGET_SWITCH_DELAY_MS: u64 = 80;
const RE_EVALUATE_INTERVAL_MS: u64 = 100;
const SWITCH_INTERP_MS: f32 = 140.0;
const RAMP_UP_MS: f32 = 280.0;
const RAMP_MIN: f32 = 0.10;

pub struct MouseAim {
    #[allow(dead_code)]
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,

    accum_dx: f32,
    accum_dy: f32,
    last_frame_time: Instant,

    locked_target: Option<LockedTarget>,
    prev_target_screen: Option<Vector2>,
    target_switch_time: Option<Instant>,
    last_reeval_time: Instant,

    velocity_trackers: HashMap<usize, RingVelocityTracker>,

    is_toggled: bool,
    last_key_state: bool,
    key_press_start: Option<Instant>,
    activation_time: Instant,
    activation_ramp: f32,

    current_target_name: Option<String>,
}

#[derive(Clone)]
struct LockedTarget {
    player_name: String,
    #[allow(dead_code)]
    model_address: u64,
    bone: BodyPart,
    lock_time: Instant,
    last_screen_pos: Vector2,
    #[allow(dead_code)]
    priority_score: f32,
}

impl MouseAim {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        Self {
            memory,
            cache,
            visengine,
            accum_dx: 0.0,
            accum_dy: 0.0,
            last_frame_time: Instant::now(),
            locked_target: None,
            prev_target_screen: None,
            target_switch_time: None,
            last_reeval_time: Instant::now(),
            velocity_trackers: HashMap::new(),
            is_toggled: false,
            last_key_state: false,
            key_press_start: None,
            activation_time: Instant::now(),
            activation_ramp: 0.0,
            current_target_name: None,
        }
    }

    pub fn get_current_target_name(&self) -> Option<&str> {
        self.current_target_name.as_deref()
    }

    pub fn update(&mut self, config: &Config, local_player_name: &str, forced_target: Option<&str>) {
        if !config.mouse_aim.enabled {
            self.reset();
            return;
        }

        // Hold-mode activation: aim key must be held
        let aim_key_pressed = Input::is_key_down(config.hotkeys.aim_key as i32);
        let key_just_pressed = aim_key_pressed && !self.last_key_state;
        let key_just_released = !aim_key_pressed && self.last_key_state;
        self.last_key_state = aim_key_pressed;

        if key_just_pressed {
            self.key_press_start = Some(Instant::now());
            self.is_toggled = true;
            self.activation_time = Instant::now();
            self.activation_ramp = RAMP_MIN;
        } else if key_just_released {
            self.key_press_start = None;
            self.is_toggled = false;
            self.activation_ramp = 0.0;
        }

        let is_active = aim_key_pressed;

        if !is_active {
            self.current_target_name = None;
            return;
        }

        let dimensions = self.visengine.get_dimensions();
        if dimensions.x <= 0.0 || dimensions.y <= 0.0 { return; }

        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);
        let view_matrix = self.visengine.get_view_matrix();
        let snapshot = self.cache.get_snapshot();

        let fov = config.mouse_aim.fov;
        let smoothing = config.mouse_aim.smoothing.max(1.0);
        let team_check = config.visuals.team_check;
        let hide_dead = config.visuals.hide_dead;
        let hide_transparent = config.visuals.hide_transparent;
        let zombies_mode = config.visuals.zombies_mode;
        let target_bone = &config.mouse_aim.target_bone;
        let prediction_enabled = config.aimbot.prediction_enabled;
        let prediction_ms = config.aimbot.prediction_amount;

        let local_team = self.cache.get_local_team_addr();

        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> = if team_check && !teammate_whitelist.is_empty() {
            snapshot.iter()
                .filter(|e| teammate_whitelist.iter().any(|n| n.eq_ignore_ascii_case(&e.name)))
                .map(|e| e.model_address)
                .collect()
        } else {
            AHashSet::new()
        };

        let score_candidates = |locked_name: Option<&str>, sticky: f32| -> Vec<(Entity, Vector2, f32)> {
            let mut scored = Vec::new();
            for entity in snapshot.iter() {
                if entity.name.eq_ignore_ascii_case(local_player_name) { continue; }
                if zombies_mode {
                    if !entity.is_game_specific { continue; }
                } else if is_teammate(entity, team_check, local_team, &teammate_addresses) {
                    continue;
                }
                if hide_dead && entity.is_dead() { continue; }
                if hide_transparent && entity.is_transparent { continue; }
                if entity.humanoid_address == 0 && !entity.is_game_specific && entity.root_part().is_none() {
                    continue;
                }

                let bone_pos = match get_bone_with_fallback(entity, target_bone) {
                    Some(p) => p,
                    None => continue,
                };
                if !bone_pos.is_valid() || bone_pos.is_near_origin(1.0) { continue; }

                let mut final_pos = bone_pos;
                if config.aimbot.ground_offset_enabled {
                    let root_y = entity.root_position().map(|p| p.y).unwrap_or(bone_pos.y);
                    if root_y <= 3.0 {
                        final_pos = Vector3 { x: bone_pos.x, y: bone_pos.y + config.aimbot.ground_offset_y, z: bone_pos.z };
                    }
                }

                let sp = match self.visengine.world_to_screen(final_pos, dimensions, &view_matrix) {
                    Some(s) => s,
                    None => continue,
                };

                let dist = sp.distance_to(screen_center);
                if dist > fov { continue; }

                let mut priority = dist;
                if entity.max_health > 0.0 {
                    let health_pct = entity.health / entity.max_health;
                    priority *= 0.5 + health_pct * 0.5;
                }
                let center_bonus = 1.0 - (dist / fov.max(1.0)).min(1.0) * 0.3;
                priority *= center_bonus;

                if let Some(locked) = locked_name {
                    if entity.name == locked { priority /= sticky.max(1.0); }
                }

                scored.push((entity.clone(), sp, priority));
            }
            scored
        };

        let sticky = 1.5f32; // sticky factor (not configurable in demo)
        let now_reeval = Instant::now();
        let should_reeval = now_reeval.duration_since(self.last_reeval_time).as_millis() as u64 >= RE_EVALUATE_INTERVAL_MS;

        // --- Global lock-on ---
        if let Some(pin) = forced_target {
            let entity = snapshot.iter().find(|e| e.name.eq_ignore_ascii_case(pin));
            match entity {
                Some(e) if !e.is_dead() => {
                    let mut bone_pos = match get_bone_with_fallback(e, target_bone) {
                        Some(p) => p,
                        None => { self.current_target_name = None; return; }
                    };
                    if config.aimbot.ground_offset_enabled {
                        let root_y = e.root_position().map(|p| p.y).unwrap_or(bone_pos.y);
                        if root_y <= 3.0 { bone_pos.y += config.aimbot.ground_offset_y; }
                    }
                    if let Some(sp) = self.visengine.world_to_screen(bone_pos, dimensions, &view_matrix) {
                        self.current_target_name = Some(e.name.clone());
                        let error_x = sp.x - screen_center.x;
                        let error_y = sp.y - screen_center.y;
                        let error_mag = (error_x * error_x + error_y * error_y).sqrt();
                        if error_mag < DEAD_ZONE_PX { return; }
                        let now = Instant::now();
                        let dt = now.duration_since(self.last_frame_time).as_secs_f32().clamp(0.0001, 0.05);
                        self.last_frame_time = now;
                        let base_speed = 0.5 / (smoothing * 0.15 + 1.0);
                        let dist_gain = if error_mag < 4.0 { 0.15 } else if error_mag < 20.0 { 0.25 } else if error_mag < 80.0 { 0.40 } else if error_mag < 250.0 { 0.50 } else { 0.55 };
                        let ramp_elapsed_ms = now.duration_since(self.activation_time).as_secs_f32() * 1000.0;
                        self.activation_ramp = if ramp_elapsed_ms >= RAMP_UP_MS { 1.0 } else {
                            let t = ramp_elapsed_ms / RAMP_UP_MS;
                            RAMP_MIN + (1.0 - RAMP_MIN) * (1.0 - (1.0 - t).powi(3))
                        };
                        let frame_factor = (base_speed * dist_gain * self.activation_ramp * dt * 60.0).min(MAX_FRAME_CORRECTION);
                        let dx = (error_x * frame_factor).clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
                        let dy = (error_y * frame_factor).clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
                        self.accum_dx += dx;
                        self.accum_dy += dy;
                        let ix = self.accum_dx as i32;
                        let iy = self.accum_dy as i32;
                        if ix != 0 || iy != 0 {
                            Input::move_mouse(ix, iy);
                            self.accum_dx -= ix as f32;
                            self.accum_dy -= iy as f32;
                        }
                    } else {
                        self.current_target_name = None;
                    }
                }
                _ => { self.current_target_name = None; }
            }
            return;
        }

        let mut target: Option<(Entity, Vector2)> = None;

        // Try locked target first
        if let Some(ref lock) = self.locked_target.clone() {
            if let Some(entity) = snapshot.iter().find(|e| e.name == lock.player_name) {
                if !(hide_dead && entity.is_dead())
                    && !(hide_transparent && entity.is_transparent)
                    && (if zombies_mode { entity.is_game_specific } else { !is_teammate(entity, team_check, local_team, &teammate_addresses) })
                {
                    if let Some(part) = entity.parts.get(&lock.bone) {
                        let mut pos = part.position;
                        if pos.is_valid() && !pos.is_near_origin(1.0) {
                            if prediction_enabled {
                                let tracker = self.velocity_trackers
                                    .entry(entity.model_address as usize)
                                    .or_insert_with(|| RingVelocityTracker::new(pos));
                                let vel = tracker.update(pos);
                                let accel = tracker.get_acceleration();
                                if vel.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                                    let t = prediction_ms;
                                    pos = pos + vel * t + accel * (0.5 * t * t);
                                }
                            }
                            if config.aimbot.ground_offset_enabled {
                                let root_y = entity.root_position().map(|p| p.y).unwrap_or(pos.y);
                                if root_y <= 3.0 { pos.y += config.aimbot.ground_offset_y; }
                            }
                            if let Some(sp) = self.visengine.world_to_screen(pos, dimensions, &view_matrix) {
                                let dist = sp.distance_to(screen_center);
                                if dist < LOCK_BREAK_DISTANCE {
                                    if should_reeval {
                                        self.last_reeval_time = now_reeval;
                                        let candidates = score_candidates(Some(&lock.player_name), sticky);
                                        if let Some((best_entity, best_sp, best_score)) = candidates.iter()
                                            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                                        {
                                            if best_entity.name != lock.player_name {
                                                let can_switch = lock.lock_time.elapsed().as_millis() as u64 > TARGET_SWITCH_DELAY_MS;
                                                if can_switch {
                                                    self.prev_target_screen = Some(sp);
                                                    self.target_switch_time = Some(now_reeval);
                                                    let bone = Self::find_bone(best_entity, target_bone).unwrap_or(BodyPart::Head);
                                                    self.locked_target = Some(LockedTarget {
                                                        player_name: best_entity.name.clone(),
                                                        model_address: best_entity.model_address,
                                                        bone,
                                                        lock_time: Instant::now(),
                                                        last_screen_pos: *best_sp,
                                                        priority_score: *best_score,
                                                    });
                                                    target = Some((best_entity.clone(), *best_sp));
                                                } else {
                                                    target = Some((entity.clone(), sp));
                                                }
                                            } else {
                                                target = Some((entity.clone(), sp));
                                            }
                                        } else {
                                            target = Some((entity.clone(), sp));
                                        }
                                    } else {
                                        target = Some((entity.clone(), sp));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Find new target if lock broke
        if target.is_none() {
            let locked_name = self.locked_target.as_ref().map(|l| l.player_name.as_str());
            let candidates = score_candidates(locked_name, sticky);

            if let Some((best_entity, best_sp, best_score)) = candidates.into_iter()
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            {
                let can_switch = self.locked_target.as_ref()
                    .map(|l| l.lock_time.elapsed().as_millis() as u64 > TARGET_SWITCH_DELAY_MS)
                    .unwrap_or(true);

                if can_switch {
                    if let Some(ref lock) = self.locked_target {
                        self.prev_target_screen = Some(lock.last_screen_pos);
                        self.target_switch_time = Some(now_reeval);
                    }
                    let bone = Self::find_bone(&best_entity, target_bone).unwrap_or(BodyPart::Head);
                    self.locked_target = Some(LockedTarget {
                        player_name: best_entity.name.clone(),
                        model_address: best_entity.model_address,
                        bone,
                        lock_time: Instant::now(),
                        last_screen_pos: best_sp,
                        priority_score: best_score,
                    });
                    target = Some((best_entity, best_sp));
                }
            }
        }

        let (entity, mut screen_pos) = match target {
            Some(t) => t,
            None => { self.current_target_name = None; return; }
        };

        self.current_target_name = Some(entity.name.clone());

        // Smooth target switch interpolation
        if let (Some(prev_screen), Some(switch_time)) = (self.prev_target_screen, self.target_switch_time) {
            let elapsed_ms = now_reeval.duration_since(switch_time).as_secs_f32() * 1000.0;
            if elapsed_ms < SWITCH_INTERP_MS {
                let t = elapsed_ms / SWITCH_INTERP_MS;
                let eased = 1.0 - (1.0 - t).powi(3);
                screen_pos = Vector2::new(
                    prev_screen.x + (screen_pos.x - prev_screen.x) * eased,
                    prev_screen.y + (screen_pos.y - prev_screen.y) * eased,
                );
            } else {
                self.prev_target_screen = None;
                self.target_switch_time = None;
            }
        }

        if let Some(ref mut lock) = self.locked_target {
            lock.last_screen_pos = screen_pos;
        }

        let error_x = screen_pos.x - screen_center.x;
        let error_y = screen_pos.y - screen_center.y;
        let error_mag = (error_x * error_x + error_y * error_y).sqrt();
        if error_mag < DEAD_ZONE_PX { return; }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32().clamp(0.0001, 0.05);
        self.last_frame_time = now;

        let base_speed = 0.5 / (smoothing * 0.15 + 1.0);
        let dist_gain = if error_mag < 4.0 { 0.15 }
            else if error_mag < 20.0 { 0.25 }
            else if error_mag < 80.0 { 0.40 }
            else if error_mag < 250.0 { 0.50 }
            else { 0.55 };

        let ramp_elapsed_ms = now.duration_since(self.activation_time).as_secs_f32() * 1000.0;
        self.activation_ramp = if ramp_elapsed_ms >= RAMP_UP_MS {
            1.0
        } else {
            let t = ramp_elapsed_ms / RAMP_UP_MS;
            RAMP_MIN + (1.0 - RAMP_MIN) * (1.0 - (1.0 - t).powi(3))
        };

        let frame_factor = (base_speed * dist_gain * self.activation_ramp * dt * 60.0).min(MAX_FRAME_CORRECTION);
        let dx = error_x * frame_factor;
        let dy = error_y * frame_factor;
        let clamped_dx = dx.clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
        let clamped_dy = dy.clamp(-MAX_MOUSE_DELTA, MAX_MOUSE_DELTA);
        self.accum_dx += clamped_dx;
        self.accum_dy += clamped_dy;

        let send_dx = self.accum_dx as i32;
        let send_dy = self.accum_dy as i32;
        if send_dx != 0 || send_dy != 0 {
            self.accum_dx -= send_dx as f32;
            self.accum_dy -= send_dy as f32;
            Input::move_mouse(send_dx, send_dy);
        }
    }

    fn reset(&mut self) {
        self.locked_target = None;
        self.current_target_name = None;
        self.is_toggled = false;
        self.accum_dx = 0.0;
        self.accum_dy = 0.0;
        self.last_frame_time = Instant::now();
        self.prev_target_screen = None;
        self.target_switch_time = None;
        self.last_reeval_time = Instant::now();
        self.key_press_start = None;
        self.activation_ramp = 0.0;
    }

    fn find_bone(entity: &Entity, preferred: &str) -> Option<BodyPart> {
        let bone = match preferred {
            "Head" => BodyPart::Head,
            "UpperTorso" | "Torso" => BodyPart::UpperTorso,
            "LowerTorso" => BodyPart::LowerTorso,
            "HumanoidRootPart" => BodyPart::HumanoidRootPart,
            _ => BodyPart::Head,
        };
        if entity.parts.contains_key(&bone) { Some(bone) }
        else if entity.parts.contains_key(&BodyPart::Head) { Some(BodyPart::Head) }
        else if entity.parts.contains_key(&BodyPart::UpperTorso) { Some(BodyPart::UpperTorso) }
        else if entity.parts.contains_key(&BodyPart::HumanoidRootPart) { Some(BodyPart::HumanoidRootPart) }
        else { None }
    }
}
