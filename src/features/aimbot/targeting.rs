#![allow(dead_code)]

use ahash::AHashSet;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::Memory;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache, Entity};
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{is_teammate, RingVelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::get_bone_with_fallback;

// ============================================================================
// Tuning Constants — Professional PD Aim Controller
// ============================================================================

const DEAD_ZONE_PX: f32 = 0.5;
const MAX_MOUSE_DELTA: f32 = 80.0;

const ZONE_MICRO: f32 = 4.0;
const ZONE_CLOSE: f32 = 20.0;
const ZONE_MEDIUM: f32 = 80.0;
const ZONE_FAR: f32 = 250.0;

const GAIN_MICRO: f32 = 0.12;
const GAIN_CLOSE: f32 = 0.22;
const GAIN_MEDIUM: f32 = 0.38;
const GAIN_FAR: f32 = 0.50;
const GAIN_SNAP: f32 = 0.55;

const KD_COEFFICIENT: f32 = 0.40;

const TARGET_SWITCH_DELAY_MS: u64 = 65;
const LOCK_BREAK_DISTANCE: f32 = 450.0;

const RAMP_UP_MS: f32 = 280.0;
const RAMP_MIN: f32 = 0.10;

const SWITCH_INTERP_MS: f32 = 140.0;

const SCREEN_LEAD_FACTOR: f32 = 0.30;

const CQC_RANGE_STUDS: f32 = 15.0;
const CQC_MIN_FACTOR: f32 = 0.20;

const HUMANIZE_VARIANCE: f32 = 0.006;
const MICRO_OVERSHOOT_CHANCE: f32 = 0.03;
const MICRO_OVERSHOOT_AMOUNT: f32 = 0.35;

// ============================================================================
// Supporting Structures
// ============================================================================

#[derive(Clone)]
struct LockedTarget {
    player_name: String,
    model_address: u64,
    locked_bone: BodyPart,
    lock_time: Instant,
    #[allow(dead_code)]
    priority_score: f32,
    last_screen_pos: Vector2,
}

struct ScreenVelocityTracker {
    prev_pos: Vector2,
    prev_time: Instant,
    velocity: Vector2,
}

impl ScreenVelocityTracker {
    fn new(pos: Vector2) -> Self {
        Self {
            prev_pos: pos,
            prev_time: Instant::now(),
            velocity: Vector2::ZERO,
        }
    }

    fn update(&mut self, pos: Vector2) -> Vector2 {
        let dt = self.prev_time.elapsed().as_secs_f32();
        if dt > 0.001 && dt < 0.15 {
            let instant = Vector2::new(
                (pos.x - self.prev_pos.x) / dt,
                (pos.y - self.prev_pos.y) / dt,
            );
            if instant.length() < 8000.0 {
                let alpha = 0.35;
                self.velocity = Vector2::new(
                    self.velocity.x + alpha * (instant.x - self.velocity.x),
                    self.velocity.y + alpha * (instant.y - self.velocity.y),
                );
            }
            self.prev_pos = pos;
            self.prev_time = Instant::now();
        }
        self.velocity
    }
}

// ============================================================================
// AimAssist
// ============================================================================

pub struct AimAssist {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    pub visengine: Arc<VisualEngine>,

    accumulated_dx: f32,
    accumulated_dy: f32,

    aimbot_locked_target: Option<LockedTarget>,

    velocity_trackers: HashMap<usize, RingVelocityTracker>,

    screen_vel_tracker: Option<ScreenVelocityTracker>,

    last_frame_time: Instant,

    prev_error: Vector2,
    prev_error_valid: bool,

    rng_state: u32,

    is_toggled_on: bool,
    last_key_state: bool,
    activation_time: Instant,
    deactivation_time: Instant,
    key_press_start: Option<Instant>,

    activation_ramp: f32,

    prev_target_screen: Option<Vector2>,
    target_switch_time: Option<Instant>,

    cached_local_pos: Vector3,
}

impl AimAssist {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        Self {
            memory,
            cache,
            visengine,
            accumulated_dx: 0.0,
            accumulated_dy: 0.0,
            aimbot_locked_target: None,
            velocity_trackers: HashMap::new(),
            screen_vel_tracker: None,
            last_frame_time: Instant::now(),
            prev_error: Vector2::ZERO,
            prev_error_valid: false,
            rng_state: 0xDEADBEEF,
            is_toggled_on: false,
            last_key_state: false,
            activation_time: Instant::now(),
            deactivation_time: Instant::now(),
            key_press_start: None,
            activation_ramp: 0.0,
            prev_target_screen: None,
            target_switch_time: None,
            cached_local_pos: Vector3::ZERO,
        }
    }

    pub fn get_current_target_name(&self) -> Option<&str> {
        self.aimbot_locked_target.as_ref().map(|l| l.player_name.as_str())
    }

    // ====================================================================
    // PRNG & Humanization
    // ====================================================================

    #[inline]
    fn fast_rand(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    #[inline]
    fn humanize(&mut self, value: f32) -> f32 {
        let variance = (self.fast_rand() - 0.5) * 2.0 * HUMANIZE_VARIANCE;
        value * (1.0 + variance)
    }

    fn get_target_with_lock(
        &mut self,
        config: &Config,
        local_player_name: &str,
        locked: Option<&LockedTarget>,
    ) -> Option<(Entity, Vector2)> {
        let fov = config.aimbot.fov;
        let target_bone = &config.aimbot.target_bone;
        let prediction_enabled = config.aimbot.prediction_enabled;
        let prediction_ms = config.aimbot.prediction_amount;
        let team_check = config.visuals.team_check;
        let hide_dead = config.visuals.hide_dead;
        let hide_transparent = config.visuals.hide_transparent;

        let snapshot = self.cache.get_snapshot();
        let view_matrix = self.visengine.get_view_matrix();
        let dimensions = self.visengine.get_dimensions();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return None;
        }

        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);

        let local_entity = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name));
        let local_team = self.cache.get_local_team_addr();

        self.cached_local_pos = local_entity
            .and_then(|e| e.root_position())
            .or_else(|| self.visengine.get_camera_position())
            .unwrap_or(Vector3::ZERO);

        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> = if team_check && !teammate_whitelist.is_empty() {
            snapshot.iter()
                .filter(|e| teammate_whitelist.iter().any(|n| n.eq_ignore_ascii_case(&e.name)))
                .map(|e| e.model_address)
                .collect()
        } else {
            AHashSet::new()
        };

        if let Some(lock) = locked {
            let lock_duration = lock.lock_time.elapsed().as_millis() as u64;
            if !lock.player_name.eq_ignore_ascii_case(local_player_name) {
                if let Some(entity) = snapshot.iter().find(|e| e.name == lock.player_name) {
                    if hide_dead && entity.is_dead() {
                        return None;
                    }
                    if hide_transparent && entity.is_transparent {
                        return None;
                    }
                    if is_teammate(entity, team_check, local_team, &teammate_addresses) {
                        // teammate — drop the lock
                    } else if let Some(part) = entity.parts.get(&lock.locked_bone) {
                        let mut target_pos = part.position;

                        if target_pos.is_valid() && !target_pos.is_near_origin(1.0) {
                            let tracker = self.velocity_trackers
                                .entry(entity.model_address as usize)
                                .or_insert_with(|| RingVelocityTracker::new(target_pos));
                            let velocity = tracker.update(target_pos);
                            let acceleration = tracker.get_acceleration();

                            if prediction_enabled && velocity.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                                target_pos = Self::predict_position_quadratic(
                                    target_pos, velocity, acceleration, prediction_ms * 1000.0,
                                );
                            }

                            if config.aimbot.ground_offset_enabled {
                                let root_y = entity.root_position().map(|p| p.y).unwrap_or(target_pos.y);
                                if root_y <= 3.0 {
                                    target_pos.y += config.aimbot.ground_offset_y;
                                }
                            }

                            if let Some(screen_pos) = self.visengine.world_to_screen(target_pos, dimensions, &view_matrix) {
                                let dist = screen_pos.distance_to(screen_center);
                                if dist < LOCK_BREAK_DISTANCE || lock_duration < TARGET_SWITCH_DELAY_MS {
                                    return Some((entity.clone(), screen_pos));
                                }
                            }
                        }
                    }
                }
            }
        }

        let candidates: Vec<(Entity, Vector2, f32)> = snapshot
            .par_iter()
            .filter_map(|entity| {
                if entity.name.eq_ignore_ascii_case(local_player_name) {
                    return None;
                }
                if is_teammate(entity, team_check, local_team, &teammate_addresses) {
                    return None;
                }
                if hide_dead && entity.is_dead() {
                    return None;
                }
                if hide_transparent && entity.is_transparent {
                    return None;
                }
                if entity.humanoid_address == 0 && entity.root_part().is_none() {
                    return None;
                }

                let target_pos = get_bone_with_fallback(entity, target_bone)?;

                if !target_pos.is_valid() || target_pos.is_near_origin(1.0) {
                    return None;
                }

                let target_pos = if config.aimbot.ground_offset_enabled {
                    let root_y = entity.root_position().map(|p| p.y).unwrap_or(target_pos.y);
                    if root_y <= 3.0 {
                        Vector3 { x: target_pos.x, y: target_pos.y + config.aimbot.ground_offset_y, z: target_pos.z }
                    } else {
                        target_pos
                    }
                } else {
                    target_pos
                };

                let screen_pos = self.visengine.world_to_screen(target_pos, dimensions, &view_matrix)?;
                let screen_dist = screen_pos.distance_to(screen_center);

                if screen_dist > fov {
                    return None;
                }

                let mut priority = screen_dist;
                if config.aimbot.prioritize_health && entity.max_health > 0.0 {
                    let health_pct = entity.health / entity.max_health;
                    priority *= 0.5 + health_pct * 0.5;
                }
                let center_bonus = 1.0 - (screen_dist / fov).min(1.0) * 0.3;
                priority *= center_bonus;

                Some((entity.clone(), screen_pos, priority))
            })
            .collect();

        candidates
            .into_iter()
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(entity, screen_pos, _)| (entity, screen_pos))
    }

    // ====================================================================
    // Main Apply
    // ====================================================================

    pub fn apply(&mut self, config: &Config, local_player_name: &str, forced_target: Option<&str>) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32().clamp(0.0001, 0.05);
        self.last_frame_time = now;

        if !config.aimbot.enabled {
            self.reset_state_full();
            return;
        }

        let aim_key_pressed = Input::is_key_down(config.hotkeys.aim_key as i32);
        let is_active = self.check_activation(config, aim_key_pressed);
        self.cache.set_high_priority(is_active);

        if !is_active {
            self.reset_state_soft();
            return;
        }

        if config.aimbot.activation_mode >= 1 && config.hotkeys.aim_key != 0x02 {
            let rmb_held = Input::is_key_down(0x02);
            if !rmb_held {
                return;
            }
        }

        let ramp_elapsed_ms = now.duration_since(self.activation_time).as_secs_f32() * 1000.0;
        self.activation_ramp = if ramp_elapsed_ms >= RAMP_UP_MS {
            1.0
        } else {
            let t = ramp_elapsed_ms / RAMP_UP_MS;
            RAMP_MIN + (1.0 - RAMP_MIN) * Self::ease_out_cubic(t)
        };

        let effective_lock: Option<LockedTarget> = if let Some(pin) = forced_target {
            self.aimbot_locked_target.clone().and_then(|l| {
                if l.player_name.eq_ignore_ascii_case(pin) { Some(l) } else { None }
            }).or_else(|| Some(LockedTarget {
                player_name: pin.to_string(),
                model_address: 0,
                locked_bone: BodyPart::Head,
                lock_time: now,
                priority_score: 0.0,
                last_screen_pos: Vector2::ZERO,
            }))
        } else {
            self.aimbot_locked_target.clone()
        };

        let target_bone = &config.aimbot.target_bone;
        let target_result = self.get_target_with_lock(
            config,
            local_player_name,
            effective_lock.as_ref(),
        );

        let (entity, mut screen_pos) = match target_result {
            Some(t) => t,
            None => {
                self.reset_state_soft();
                return;
            }
        };

        let local_pos = self.cached_local_pos;
        let target_pos = entity.parts.get(&BodyPart::HumanoidRootPart)
            .or_else(|| entity.parts.get(&BodyPart::UpperTorso))
            .or_else(|| entity.parts.get(&BodyPart::Head))
            .map(|p| p.position)
            .unwrap_or(Vector3::ZERO);
        let world_distance = local_pos.distance_to(target_pos);

        let dimensions = self.visengine.get_dimensions();
        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);
        let priority_score = screen_pos.distance_to(screen_center);

        let is_new_target = match &self.aimbot_locked_target {
            Some(lock) => lock.player_name != entity.name,
            None => true,
        };

        if is_new_target {
            if let Some(ref lock) = self.aimbot_locked_target {
                self.prev_target_screen = Some(lock.last_screen_pos);
                self.target_switch_time = Some(now);
            }

            let locked_bone = Self::find_valid_bone(&entity, target_bone)
                .unwrap_or(BodyPart::Head);
            self.aimbot_locked_target = Some(LockedTarget {
                player_name: entity.name.clone(),
                model_address: entity.model_address,
                locked_bone,
                lock_time: now,
                priority_score,
                last_screen_pos: screen_pos,
            });

            self.screen_vel_tracker = Some(ScreenVelocityTracker::new(screen_pos));
            self.prev_error_valid = false;
        } else if let Some(ref mut lock) = self.aimbot_locked_target {
            lock.last_screen_pos = screen_pos;
            if lock.model_address != entity.model_address {
                lock.model_address = entity.model_address;
                lock.lock_time = now;
            }
        }

        if let (Some(prev_screen), Some(switch_time)) = (self.prev_target_screen, self.target_switch_time) {
            let elapsed_ms = now.duration_since(switch_time).as_secs_f32() * 1000.0;
            if elapsed_ms < SWITCH_INTERP_MS {
                let t = Self::ease_out_cubic(elapsed_ms / SWITCH_INTERP_MS);
                screen_pos = prev_screen.lerp(screen_pos, t);
            } else {
                self.prev_target_screen = None;
                self.target_switch_time = None;
            }
        }

        if let Some(ref mut tracker) = self.screen_vel_tracker {
            let screen_vel = tracker.update(screen_pos);
            let smoothing = config.aimbot.smoothing.max(1.0);
            let lead_time_sec = smoothing * 0.004;
            if screen_vel.length() > 30.0 {
                screen_pos = Vector2::new(
                    screen_pos.x + screen_vel.x * lead_time_sec * SCREEN_LEAD_FACTOR,
                    screen_pos.y + screen_vel.y * lead_time_sec * SCREEN_LEAD_FACTOR,
                );
            }
        }

        self.apply_pd_aim(screen_pos, world_distance, dt, config);
    }

    // ====================================================================
    // PD Aim Controller
    // ====================================================================

    fn apply_pd_aim(
        &mut self,
        target_screen: Vector2,
        world_distance: f32,
        dt: f32,
        config: &Config,
    ) {
        let dimensions = self.visengine.get_dimensions();
        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);

        let error = Vector2::new(
            target_screen.x - screen_center.x,
            target_screen.y - screen_center.y,
        );
        let error_mag = error.length();

        if error_mag < DEAD_ZONE_PX {
            self.accumulated_dx = 0.0;
            self.accumulated_dy = 0.0;
            self.prev_error = error;
            self.prev_error_valid = true;
            return;
        }

        let smoothing = config.aimbot.smoothing.max(1.0);
        let base_speed = 0.5 / (smoothing * 0.15 + 1.0);

        let zone_gain = if error_mag < ZONE_MICRO {
            GAIN_MICRO
        } else if error_mag < ZONE_CLOSE {
            let t = (error_mag - ZONE_MICRO) / (ZONE_CLOSE - ZONE_MICRO);
            GAIN_MICRO + (GAIN_CLOSE - GAIN_MICRO) * t
        } else if error_mag < ZONE_MEDIUM {
            let t = (error_mag - ZONE_CLOSE) / (ZONE_MEDIUM - ZONE_CLOSE);
            GAIN_CLOSE + (GAIN_MEDIUM - GAIN_CLOSE) * Self::ease_out_quad(t)
        } else if error_mag < ZONE_FAR {
            let t = (error_mag - ZONE_MEDIUM) / (ZONE_FAR - ZONE_MEDIUM);
            GAIN_MEDIUM + (GAIN_FAR - GAIN_MEDIUM) * Self::ease_out_cubic(t)
        } else {
            GAIN_SNAP
        };

        let cqc_factor = if world_distance < CQC_RANGE_STUDS {
            CQC_MIN_FACTOR + (1.0 - CQC_MIN_FACTOR) * (world_distance / CQC_RANGE_STUDS)
        } else {
            1.0
        };

        let ramp = self.activation_ramp;

        let p_speed = base_speed * zone_gain * cqc_factor * ramp;
        let frame_p = (p_speed * dt * 60.0).min(0.45);
        let p_x = error.x * frame_p;
        let p_y = error.y * frame_p;

        let (d_x, d_y) = if self.prev_error_valid {
            let de_x = error.x - self.prev_error.x;
            let de_y = error.y - self.prev_error.y;
            let d_scale = KD_COEFFICIENT * zone_gain;
            (-de_x * d_scale, -de_y * d_scale)
        } else {
            (0.0, 0.0)
        };

        self.prev_error = error;
        self.prev_error_valid = true;

        let mut move_x = p_x + d_x;
        let mut move_y = p_y + d_y;

        move_x = self.humanize(move_x);
        move_y = self.humanize(move_y);

        if self.fast_rand() < MICRO_OVERSHOOT_CHANCE && error_mag < ZONE_CLOSE {
            let overshoot = (self.fast_rand() - 0.5) * 2.0 * MICRO_OVERSHOOT_AMOUNT;
            move_x += overshoot;
            move_y += overshoot * 0.7;
        }

        if config.aimbot.sens_compensation {
            let sens_offset = crate::core::offsets::mouse_service::sensitivity_pointer();
            if sens_offset != 0 {
                let game_sens: f32 = self.memory.read(self.memory.base_address() + sens_offset);
                if game_sens > 0.01 {
                    let compensation = 1.0 / (game_sens + 0.2);
                    move_x *= compensation;
                    move_y *= compensation;
                }
            }
        }

        self.accumulated_dx += move_x;
        self.accumulated_dy += move_y;

        let int_dx = self.accumulated_dx.trunc() as i32;
        let int_dy = self.accumulated_dy.trunc() as i32;
        self.accumulated_dx -= int_dx as f32;
        self.accumulated_dy -= int_dy as f32;

        let int_dx = int_dx.clamp(-(MAX_MOUSE_DELTA as i32), MAX_MOUSE_DELTA as i32);
        let int_dy = int_dy.clamp(-(MAX_MOUSE_DELTA as i32), MAX_MOUSE_DELTA as i32);

        if int_dx != 0 || int_dy != 0 {
            Input::move_mouse(int_dx, int_dy);
        }
    }

    // ====================================================================
    // Activation System
    // ====================================================================

    fn check_activation(&mut self, config: &Config, key_pressed: bool) -> bool {
        let key_just_pressed = key_pressed && !self.last_key_state;
        let key_just_released = !key_pressed && self.last_key_state;
        self.last_key_state = key_pressed;

        let hold_delay_ms = config.aimbot.hold_delay_ms;

        match config.aimbot.activation_mode {
            0 => {
                if key_just_pressed {
                    self.key_press_start = Some(Instant::now());
                } else if key_just_released {
                    self.key_press_start = None;
                    if self.is_toggled_on {
                        self.is_toggled_on = false;
                        self.deactivation_time = Instant::now();
                    }
                    return false;
                }
                if key_pressed {
                    if let Some(start) = self.key_press_start {
                        let held_ms = start.elapsed().as_millis() as u32;
                        if held_ms >= hold_delay_ms {
                            if !self.is_toggled_on {
                                self.is_toggled_on = true;
                                self.activation_time = Instant::now();
                                self.activation_ramp = RAMP_MIN;
                            }
                            return true;
                        }
                    }
                }
                false
            }
            1 => {
                if key_just_pressed {
                    self.is_toggled_on = !self.is_toggled_on;
                    if self.is_toggled_on {
                        self.activation_time = Instant::now();
                        self.activation_ramp = RAMP_MIN;
                    } else {
                        self.deactivation_time = Instant::now();
                    }
                }
                self.is_toggled_on
            }
            2 => {
                if !self.is_toggled_on {
                    self.is_toggled_on = true;
                    self.activation_time = Instant::now();
                    self.activation_ramp = RAMP_MIN;
                }
                true
            }
            _ => key_pressed,
        }
    }

    // ====================================================================
    // Easing Functions
    // ====================================================================

    #[inline]
    fn ease_out_quad(t: f32) -> f32 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    #[inline]
    fn ease_out_cubic(t: f32) -> f32 {
        let t1 = 1.0 - t;
        1.0 - t1 * t1 * t1
    }

    // ====================================================================
    // State Management
    // ====================================================================

    pub fn get_locked_target_name(&self) -> Option<&str> {
        self.aimbot_locked_target.as_ref().map(|t| t.player_name.as_str())
    }

    pub fn get_local_pos(&self) -> Vector3 {
        self.cached_local_pos
    }

    pub fn get_locked_target_world_pos(&self) -> Option<Vector3> {
        let lock = self.aimbot_locked_target.as_ref()?;
        let snapshot = self.cache.get_snapshot();
        snapshot.iter()
            .find(|e| e.name == lock.player_name)
            .and_then(|e| e.root_position())
    }

    fn reset_state_soft(&mut self) {
        self.accumulated_dx = 0.0;
        self.accumulated_dy = 0.0;
        self.aimbot_locked_target = None;
        self.screen_vel_tracker = None;
        self.prev_error_valid = false;
        self.prev_target_screen = None;
        self.target_switch_time = None;
    }

    fn reset_state_full(&mut self) {
        self.reset_state_soft();
        self.velocity_trackers.clear();
        self.is_toggled_on = false;
        self.activation_ramp = 0.0;
    }

    // ====================================================================
    // Bone Helpers & Prediction
    // ====================================================================

    fn find_valid_bone(entity: &Entity, preferred_bone: &str) -> Option<BodyPart> {
        let primary_bones: Vec<BodyPart> = match preferred_bone {
            "Head" => vec![BodyPart::Head],
            "Torso" | "UpperTorso" => vec![BodyPart::UpperTorso, BodyPart::Torso],
            "HumanoidRootPart" => vec![BodyPart::HumanoidRootPart],
            _ => vec![BodyPart::Head],
        };

        for bone in primary_bones {
            if let Some(part) = entity.parts.get(&bone) {
                if part.position.is_valid() && !part.position.is_near_origin(1.0) {
                    return Some(bone);
                }
            }
        }

        const FALLBACKS: [BodyPart; 4] = [
            BodyPart::Head,
            BodyPart::UpperTorso,
            BodyPart::Torso,
            BodyPart::HumanoidRootPart,
        ];
        for bone in FALLBACKS {
            if let Some(part) = entity.parts.get(&bone) {
                if part.position.is_valid() && !part.position.is_near_origin(1.0) {
                    return Some(bone);
                }
            }
        }

        None
    }

    #[inline]
    fn predict_position_quadratic(
        current: Vector3,
        velocity: Vector3,
        acceleration: Vector3,
        prediction_ms: f32,
    ) -> Vector3 {
        if velocity.length_squared() < VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
            return current;
        }

        let t = prediction_ms / 1000.0;
        let t2 = t * t;
        let max_accel = 50.0;
        let ax = acceleration.x.clamp(-max_accel, max_accel) * 0.5;
        let ay = acceleration.y.clamp(-max_accel, max_accel) * 0.5;
        let az = acceleration.z.clamp(-max_accel, max_accel) * 0.5;

        Vector3::new(
            current.x + velocity.x * t + 0.5 * ax * t2,
            current.y + velocity.y * t + 0.5 * ay * t2,
            current.z + velocity.z * t + 0.5 * az * t2,
        )
    }
}
