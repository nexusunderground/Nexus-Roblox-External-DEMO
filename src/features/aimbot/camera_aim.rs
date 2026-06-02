//! Camera Aim - CFrame Rotation Spoofing Implementation

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::memory::Memory;
use crate::core::offsets::camera;
use crate::sdk::VisualEngine;
use crate::utils::cache::Cache;
use crate::utils::input::{Input, SyntheticInputSource};
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{VelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::{TargetContext, get_bone_with_fallback, compute_priority};

// ============================================================================
// Constants
// ============================================================================

const SPOOF_WRITE_CYCLES: u32 = 7;
const CONTINUOUS_WRITE_CYCLES: u32 = 4;
const MIN_TARGET_DISTANCE: f32 = 1.0;
const MAX_TARGET_DISTANCE: f32 = 1000.0;
const MAX_VERIFY_RETRIES: u32 = 2;

// ============================================================================
// CFrame Structure
// ============================================================================

#[derive(Clone, Copy, Debug)]
pub struct CFrame {
    pub rotation: [f32; 9],
    pub position: Vector3,
}

impl CFrame {
    pub fn look_at(from: Vector3, to: Vector3) -> Self {
        let look_x = to.x - from.x;
        let look_y = to.y - from.y;
        let look_z = to.z - from.z;

        let look_len = (look_x * look_x + look_y * look_y + look_z * look_z).sqrt();
        if look_len < 0.0001 {
            return Self {
                rotation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                position: from,
            };
        }

        let lx = look_x / look_len;
        let ly = look_y / look_len;
        let lz = look_z / look_len;

        let world_up = (0.0f32, 1.0f32, 0.0f32);

        let mut rx = ly * world_up.2 - lz * world_up.1;
        let mut ry = lz * world_up.0 - lx * world_up.2;
        let mut rz = lx * world_up.1 - ly * world_up.0;

        let r_len = (rx * rx + ry * ry + rz * rz).sqrt();
        if r_len < 0.0001 {
            rx = 1.0; ry = 0.0; rz = 0.0;
        } else {
            rx /= r_len; ry /= r_len; rz /= r_len;
        }

        let ux = ry * lz - rz * ly;
        let uy = rz * lx - rx * lz;
        let uz = rx * ly - ry * lx;

        let zx = -lx;
        let zy = -ly;
        let zz = -lz;

        Self {
            rotation: [rx, ux, zx, ry, uy, zy, rz, uz, zz],
            position: from,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.rotation.iter().all(|&v| v.is_finite())
            && self.position.x.is_finite()
            && self.position.y.is_finite()
            && self.position.z.is_finite()
    }
}

// ============================================================================
// Camera Aim
// ============================================================================

pub struct CameraAim {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    #[allow(dead_code)]
    base_address: u64,

    last_shot_time: Instant,
    last_mouse_state: bool,
    is_spoofing: bool,

    original_cframe: Option<CFrame>,
    spoof_camera_addr: u64,

    current_interp_rotation: Option<[f32; 9]>,

    shot_count: u32,
    #[allow(dead_code)]
    hit_count: u32,
    write_verify_failures: u32,

    velocity_trackers: std::collections::HashMap<u64, VelocityTracker>,

    last_target_addr: u64,
    #[allow(dead_code)]
    last_target_health: f32,

    current_target_name: Option<String>,
}

impl CameraAim {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        let base_address = memory.base_address();
        Self {
            memory,
            cache,
            visengine,
            base_address,
            last_shot_time: Instant::now(),
            last_mouse_state: false,
            is_spoofing: false,
            original_cframe: None,
            spoof_camera_addr: 0,
            current_interp_rotation: None,
            shot_count: 0,
            hit_count: 0,
            write_verify_failures: 0,
            velocity_trackers: std::collections::HashMap::new(),
            last_target_addr: 0,
            last_target_health: 0.0,
            current_target_name: None,
        }
    }

    pub fn get_current_target_name(&self) -> Option<&str> {
        self.current_target_name.as_deref()
    }

    pub fn update(&mut self, config: &Config, local_player_name: &str, forced_target: Option<&str>) {
        crate::perf_scope!("camera_aim_update");
        if !config.camera_aim.enabled {
            if self.is_spoofing { self.restore_original_cframe(); }
            return;
        }

        let mouse_down = Input::is_key_down(0x01);

        if !mouse_down && self.is_spoofing {
            self.restore_original_cframe();
            self.last_mouse_state = false;
            return;
        }

        if !mouse_down {
            self.last_mouse_state = false;
            return;
        }

        let shot_initiated = mouse_down && !self.last_mouse_state;
        self.last_mouse_state = mouse_down;

        let camera_addr = match self.get_camera_address() {
            Some(addr) => addr,
            None => { if self.is_spoofing { self.restore_original_cframe(); } return; }
        };

        let camera_pos = self.read_camera_position(camera_addr);
        if camera_pos.is_near_origin(1.0) {
            if self.is_spoofing { self.restore_original_cframe(); }
            return;
        }

        let target_pos = match self.find_best_target(config, local_player_name, &camera_pos, forced_target) {
            Some(pos) => pos,
            None => {
                self.current_target_name = None;
                if self.is_spoofing { self.restore_original_cframe(); }
                return;
            }
        };

        let target_distance = camera_pos.distance_to(target_pos);
        if target_distance < MIN_TARGET_DISTANCE {
            if self.is_spoofing { self.restore_original_cframe(); }
            return;
        }

        match self.handle_continuous_spoof(camera_addr, camera_pos, target_pos, shot_initiated) {
            Ok(_) => {
                if shot_initiated {
                    self.last_shot_time = Instant::now();
                    self.shot_count += 1;
                }
            }
            Err(_) => {}
        }
    }

    fn restore_original_cframe(&mut self) {
        if let Some(original) = self.original_cframe.take() {
            if self.spoof_camera_addr != 0 {
                let rot_base = self.spoof_camera_addr + camera::rotation();
                self.memory.write::<[f32; 9]>(rot_base, original.rotation);
            }
        }
        self.is_spoofing = false;
        self.spoof_camera_addr = 0;
        self.current_interp_rotation = None;
    }

    fn handle_continuous_spoof(
        &mut self,
        camera_addr: u64,
        camera_pos: Vector3,
        target_pos: Vector3,
        is_new_shot: bool,
    ) -> Result<(), String> {
        let rot_base = camera_addr + camera::rotation();

        if !self.is_spoofing {
            let original = self.read_cframe(camera_addr);
            if !original.is_valid() {
                return Err("Invalid original CFrame".to_string());
            }
            self.original_cframe = Some(original);
            self.spoof_camera_addr = camera_addr;
            self.is_spoofing = true;
            self.current_interp_rotation = Some(original.rotation);
        }

        let target_cframe = CFrame::look_at(camera_pos, target_pos);
        if !target_cframe.is_valid() {
            return Err("Invalid spoofed CFrame".to_string());
        }

        let final_rotation = {
            let mut result = target_cframe.rotation;
            Self::normalize_rotation_matrix(&mut result);
            result
        };

        self.current_interp_rotation = Some(final_rotation);

        let cycles = if is_new_shot { SPOOF_WRITE_CYCLES } else { CONTINUOUS_WRITE_CYCLES };
        for _ in 0..cycles {
            self.memory.write::<[f32; 9]>(rot_base, final_rotation);
        }

        if is_new_shot {
            let readback = self.memory.read::<f32>(rot_base);
            if (readback - final_rotation[0]).abs() > 0.01 {
                self.write_verify_failures += 1;
                for _ in 0..MAX_VERIFY_RETRIES {
                    self.memory.write::<[f32; 9]>(rot_base, final_rotation);
                    let check = self.memory.read::<f32>(rot_base);
                    if (check - final_rotation[0]).abs() < 0.01 { break; }
                }
            }
        }

        Ok(())
    }

    fn normalize_rotation_matrix(m: &mut [f32; 9]) {
        let len0 = (m[0]*m[0] + m[3]*m[3] + m[6]*m[6]).sqrt();
        if len0 > 0.0001 { m[0] /= len0; m[3] /= len0; m[6] /= len0; }
        let len1 = (m[1]*m[1] + m[4]*m[4] + m[7]*m[7]).sqrt();
        if len1 > 0.0001 { m[1] /= len1; m[4] /= len1; m[7] /= len1; }
        let len2 = (m[2]*m[2] + m[5]*m[5] + m[8]*m[8]).sqrt();
        if len2 > 0.0001 { m[2] /= len2; m[5] /= len2; m[8] /= len2; }
    }

    fn find_best_target(
        &mut self,
        config: &Config,
        local_player_name: &str,
        camera_pos: &Vector3,
        forced_target: Option<&str>,
    ) -> Option<Vector3> {
        let ctx = TargetContext::build(&self.cache, &self.visengine, config, local_player_name)?;
        let view_matrix = self.visengine.get_view_matrix();

        if let Some(pin) = forced_target {
            let entity = ctx.snapshot.iter().find(|e| e.name.eq_ignore_ascii_case(pin))?;
            if entity.is_dead() {
                self.current_target_name = None;
                return None;
            }
            let mut hitbox_pos = get_bone_with_fallback(entity, &config.camera_aim.target_bone)?;
            if config.aimbot.prediction_enabled {
                let tracker_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                let tracker = self.velocity_trackers.entry(tracker_key).or_insert_with(|| VelocityTracker::new(hitbox_pos));
                let (velocity, _accel) = tracker.update(hitbox_pos);
                if velocity.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                    hitbox_pos = tracker.predict(hitbox_pos, config.aimbot.prediction_amount);
                }
            }
            if config.aimbot.ground_offset_enabled {
                let root_y = entity.root_position().map(|p| p.y).unwrap_or(hitbox_pos.y);
                if root_y <= 3.0 { hitbox_pos.y += config.aimbot.ground_offset_y; }
            }
            let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
            self.last_target_addr = target_key;
            self.last_target_health = entity.health;
            self.current_target_name = Some(entity.name.clone());
            return Some(hitbox_pos);
        }

        let fov = config.camera_aim.fov;
        let mut best: Option<(Vector3, f32, u64, f32, String)> = None;

        for entity in ctx.snapshot.iter() {
            if ctx.should_skip(entity, local_player_name, config.visuals.team_check, config.visuals.hide_dead) {
                continue;
            }

            let entity_pos = entity.root_position().unwrap_or(Vector3::ZERO);
            let world_dist = camera_pos.distance_to(entity_pos);
            if world_dist > MAX_TARGET_DISTANCE { continue; }

            let mut hitbox_pos = match get_bone_with_fallback(entity, &config.camera_aim.target_bone) {
                Some(pos) => pos,
                None => continue,
            };

            if config.aimbot.prediction_enabled {
                let tracker_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                let tracker = self.velocity_trackers.entry(tracker_key).or_insert_with(|| VelocityTracker::new(hitbox_pos));
                let (velocity, _accel) = tracker.update(hitbox_pos);
                if velocity.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                    hitbox_pos = tracker.predict(hitbox_pos, config.aimbot.prediction_amount);
                }
            }

            if config.aimbot.ground_offset_enabled {
                let root_y = entity.root_position().map(|p| p.y).unwrap_or(hitbox_pos.y);
                if root_y <= 3.0 { hitbox_pos.y += config.aimbot.ground_offset_y; }
            }

            if !self.is_in_fov(hitbox_pos, fov, ctx.screen_center, ctx.dimensions, &view_matrix.m) {
                continue;
            }

            let screen_pos = match self.visengine.world_to_screen(hitbox_pos, ctx.dimensions, &view_matrix) {
                Some(pos) => pos,
                None => continue,
            };

            let screen_dist = screen_pos.distance_to(ctx.screen_center);
            let priority = compute_priority(entity, screen_dist, world_dist, config.aimbot.prioritize_health);

            if best.is_none() || priority < best.as_ref().unwrap().1 {
                let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                best = Some((hitbox_pos, priority, target_key, entity.health, entity.name.clone()));
            }
        }

        if let Some((pos, _priority, addr, health, name)) = best {
            self.last_target_addr = addr;
            self.last_target_health = health;
            self.current_target_name = Some(name);
            return Some(pos);
        }

        self.current_target_name = None;
        None
    }

    fn is_in_fov(
        &self,
        world_pos: Vector3,
        fov_radius: f32,
        screen_center: Vector2,
        dimensions: Vector2,
        view_matrix: &[[f32; 4]; 4],
    ) -> bool {
        let matrix = crate::utils::math::Matrix4 { m: *view_matrix };
        match self.visengine.world_to_screen(world_pos, dimensions, &matrix) {
            Some(screen_pos) => screen_pos.distance_to(screen_center) <= fov_radius,
            None => false,
        }
    }

    fn read_cframe(&self, camera_addr: u64) -> CFrame {
        let rot_base = camera_addr + camera::rotation();
        let pos_base = camera_addr + camera::position();
        let rotation: [f32; 9] = self.memory.read(rot_base);
        let position: Vector3 = self.memory.read(pos_base);
        CFrame { rotation, position }
    }

    fn read_camera_position(&self, camera_addr: u64) -> Vector3 {
        let pos_base = camera_addr + camera::position();
        self.memory.read::<Vector3>(pos_base)
    }

    #[inline]
    fn get_camera_address(&self) -> Option<u64> {
        self.memory.resolve_camera_address()
    }

    pub fn trigger_fire(
        &mut self,
        config: &Config,
        local_player_name: &str,
        input_source: SyntheticInputSource,
    ) {
        let camera_addr = match self.get_camera_address() {
            Some(addr) => addr,
            None => return,
        };

        let camera_pos = self.read_camera_position(camera_addr);
        if camera_pos.is_near_origin(1.0) { return; }

        let target_pos = match self.find_best_target(config, local_player_name, &camera_pos, None) {
            Some(pos) => pos,
            None => return,
        };

        if camera_pos.distance_to(target_pos) < MIN_TARGET_DISTANCE { return; }

        let rot_base = camera_addr + camera::rotation();
        let original = self.read_cframe(camera_addr);
        if !original.is_valid() { return; }

        let target_cframe = CFrame::look_at(camera_pos, target_pos);
        if !target_cframe.is_valid() { return; }
        let mut final_rotation = target_cframe.rotation;
        Self::normalize_rotation_matrix(&mut final_rotation);

        for _ in 0..SPOOF_WRITE_CYCLES {
            self.memory.write::<[f32; 9]>(rot_base, final_rotation);
        }

        Input::mouse_down_from(input_source);

        const HOLD_MS: u128 = 40;
        const WRITE_INTERVAL_US: u64 = 2000;
        let hold_start = Instant::now();
        while hold_start.elapsed().as_millis() < HOLD_MS {
            self.memory.write::<[f32; 9]>(rot_base, final_rotation);
            std::thread::sleep(std::time::Duration::from_micros(WRITE_INTERVAL_US));
        }

        Input::mouse_up_from(input_source);

        self.memory.write::<[f32; 9]>(rot_base, original.rotation);

        self.shot_count += 1;
        self.last_shot_time = Instant::now();
    }
}

