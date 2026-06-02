//! Viewport Aim - Universal Silent Aim via Viewport Manipulation

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{camera, player};
use crate::sdk::{Instance, VisualEngine};
use crate::utils::cache::{BodyPart, Cache};
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{VelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::{TargetContext, get_bone_with_fallback, compute_priority};

const VIEWPORT_OFFSET: u64 = 0x2AC;

#[inline]
fn get_viewport_offset() -> u64 {
    VIEWPORT_OFFSET
}

const MIN_TARGET_DISTANCE: f32 = 1.0;
const MAX_TARGET_DISTANCE: f32 = 1000.0;
const MAX_VERIFY_RETRIES: u32 = 2;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector2int16 {
    pub x: i16,
    pub y: i16,
}

impl Vector2int16 {
    pub fn calculate_for_target(target_screen: Vector2, screen_size: Vector2) -> Self {
        Self {
            x: (2.0 * (screen_size.x - target_screen.x)) as i16,
            y: (2.0 * (screen_size.y - target_screen.y)) as i16,
        }
    }

    pub fn from_screen_size(width: f32, height: f32) -> Self {
        Self { x: width as i16, y: height as i16 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBone {
    Head = 0,
    UpperTorso = 1,
    LowerTorso = 2,
    HumanoidRootPart = 3,
}

impl TargetBone {
    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::Head,
            1 => Self::UpperTorso,
            2 => Self::LowerTorso,
            _ => Self::HumanoidRootPart,
        }
    }

    pub fn to_body_part(self) -> BodyPart {
        match self {
            Self::Head => BodyPart::Head,
            Self::UpperTorso => BodyPart::UpperTorso,
            Self::LowerTorso => BodyPart::LowerTorso,
            Self::HumanoidRootPart => BodyPart::HumanoidRootPart,
        }
    }
}

pub struct ViewportAim {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    players_instance: Arc<Instance>,

    is_spoofing: bool,
    camera_addr: u64,
    last_mouse_state: bool,
    velocity_trackers: HashMap<u64, VelocityTracker>,
    tracker_prune_counter: u32,

    shot_count: u32,
    last_target_addr: u64,
    write_verify_failures: u32,

    current_target_name: Option<String>,
    locked_target_addr: u64,
    locked_target_name: Option<String>,
    lock_break_frames: u32,

    cached_local_player_addr: u64,
    cached_character_addr: u64,
    last_tool_check: Option<std::time::Instant>,
    is_holding_grenade: bool,
}

impl ViewportAim {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>, players_instance: Arc<Instance>) -> Self {
        Self {
            memory,
            cache,
            visengine,
            players_instance,
            is_spoofing: false,
            camera_addr: 0,
            last_mouse_state: false,
            velocity_trackers: HashMap::new(),
            tracker_prune_counter: 0,
            shot_count: 0,
            last_target_addr: 0,
            write_verify_failures: 0,
            current_target_name: None,
            locked_target_addr: 0,
            locked_target_name: None,
            lock_break_frames: 0,
            cached_local_player_addr: 0,
            cached_character_addr: 0,
            last_tool_check: None,
            is_holding_grenade: false,
        }
    }

    fn read_viewport(&self, camera_addr: u64) -> Vector2int16 {
        let addr = camera_addr + get_viewport_offset();
        if is_valid_address(addr) {
            let packed = self.memory.read::<u32>(addr);
            Vector2int16 {
                x: (packed & 0xFFFF) as i16,
                y: ((packed >> 16) & 0xFFFF) as i16,
            }
        } else {
            Vector2int16::default()
        }
    }

    fn write_viewport(&mut self, camera_addr: u64, viewport: Vector2int16) {
        let addr = camera_addr + get_viewport_offset();
        if is_valid_address(addr) {
            let vx = viewport.x;
            let vy = viewport.y;
            let packed: u32 = (vx as u16 as u32) | ((vy as u16 as u32) << 16);
            self.memory.write(addr, packed);
            let readback = self.memory.read::<u32>(addr);
            if readback != packed {
                self.write_verify_failures += 1;
                for _ in 0..MAX_VERIFY_RETRIES {
                    self.memory.write(addr, packed);
                    if self.memory.read::<u32>(addr) == packed { break; }
                }
            }
        }
    }

    fn reset_viewport(&mut self) {
        if self.is_spoofing && self.camera_addr != 0 {
            let dims = self.visengine.get_dimensions();
            let normal_viewport = Vector2int16::from_screen_size(dims.x, dims.y);
            self.write_viewport(self.camera_addr, normal_viewport);
            self.is_spoofing = false;
        }
        self.locked_target_addr = 0;
        self.locked_target_name = None;
        self.lock_break_frames = 0;
    }

    fn check_holding_grenade(&mut self, local_player_name: &str) {
        if let Some(last) = self.last_tool_check {
            if last.elapsed().as_millis() < 200 { return; }
        }
        self.last_tool_check = Some(std::time::Instant::now());

        if self.cached_local_player_addr != 0 && !is_valid_address(self.cached_local_player_addr) {
            self.cached_local_player_addr = 0;
            self.cached_character_addr = 0;
        }
        if self.cached_local_player_addr == 0 {
            let lp = self.players_instance.get_children().into_iter()
                .find(|child| child.get_name().eq_ignore_ascii_case(local_player_name));
            match lp {
                Some(p) => self.cached_local_player_addr = p.address,
                None => { self.is_holding_grenade = false; return; }
            }
        }

        let character_addr = self.memory.read::<u64>(self.cached_local_player_addr + player::model_instance());
        if !is_valid_address(character_addr) { self.is_holding_grenade = false; return; }

        if character_addr != self.cached_character_addr {
            self.cached_character_addr = character_addr;
        }

        let character = Instance::new(character_addr, Arc::clone(&self.memory));
        let tool = character.get_children().into_iter().find(|child| {
            let class = child.get_class_name();
            class == "Tool" || class == "HopperBin"
        });

        self.is_holding_grenade = match tool {
            Some(t) => {
                let name = t.get_name().to_lowercase();
                name.contains("grenade") || name.contains("frag") || name.contains("smoke")
                    || name.contains("flash") || name.contains("molotov") || name.contains("throwable")
                    || name.contains("explosive") || name.contains("c4") || name.contains("dynamite")
            }
            None => false,
        };
    }

    fn get_camera_position(&self, camera_addr: u64) -> Vector3 {
        let pos_addr = camera_addr + camera::position();
        if is_valid_address(pos_addr) {
            self.memory.read::<Vector3>(pos_addr)
        } else {
            Vector3::ZERO
        }
    }

    fn find_target(
        &mut self,
        config: &Config,
        camera_pos: Vector3,
        screen_center: Vector2,
        dims: Vector2,
        local_player_name: &str,
        forced_target: Option<&str>,
    ) -> Option<(u64, Vector3, Vector2, String)> {
        let ctx = TargetContext::build(&self.cache, &self.visengine, config, local_player_name)?;
        let target_bone = TargetBone::Head; // demo: always aim at head
        let view_matrix = self.visengine.get_view_matrix();

        if let Some(pin) = forced_target {
            let entity = ctx.snapshot.iter().find(|e| e.name.eq_ignore_ascii_case(pin))?;
            if entity.is_dead() { return None; }
            let mut bone_pos = get_bone_with_fallback(entity, target_bone.to_body_part().to_name())?;
            if config.aimbot.ground_offset_enabled {
                let root_y = entity.root_position().map(|p| p.y).unwrap_or(bone_pos.y);
                if root_y <= 3.0 { bone_pos.y += config.aimbot.ground_offset_y; }
            }
            let screen_pos = self.visengine.world_to_screen(bone_pos, dims, &view_matrix)?;
            let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
            return Some((target_key, bone_pos, screen_pos, entity.name.clone()));
        }

        let mut best_target: Option<(u64, Vector3, Vector2, f32, String)> = None;

        for entity in ctx.snapshot.iter() {
            if ctx.should_skip(entity, local_player_name, config.visuals.team_check, true) {
                continue;
            }

            let mut bone_pos = match get_bone_with_fallback(entity, target_bone.to_body_part().to_name()) {
                Some(pos) => pos,
                None => continue,
            };

            if config.aimbot.ground_offset_enabled {
                let root_y = entity.root_position().map(|p| p.y).unwrap_or(bone_pos.y);
                if root_y <= 3.0 { bone_pos.y += config.aimbot.ground_offset_y; }
            }

            let distance = camera_pos.distance_to(bone_pos);
            if distance < MIN_TARGET_DISTANCE || distance > MAX_TARGET_DISTANCE { continue; }

            let screen_pos = match self.visengine.world_to_screen(bone_pos, dims, &view_matrix) {
                Some(pos) => pos,
                None => continue,
            };

            let dist_to_crosshair = screen_pos.distance_to(screen_center);

            if dist_to_crosshair > config.viewport_aim.fov { continue; }

            let priority = compute_priority(entity, dist_to_crosshair, distance, config.aimbot.prioritize_health);

            if best_target.is_none() || priority < best_target.as_ref().unwrap().3 {
                let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                best_target = Some((target_key, bone_pos, screen_pos, priority, entity.name.clone()));
            }
        }

        let result = best_target.map(|(addr, pos, screen, _, name)| (addr, pos, screen, name));

        let locked_entity_current: Option<(u64, Vector3, Vector2, String)> = if self.locked_target_addr != 0 {
            ctx.snapshot.iter()
                .find(|e| {
                    let k = if e.humanoid_address != 0 { e.humanoid_address } else { e.model_address };
                    k == self.locked_target_addr && !e.is_dead()
                })
                .and_then(|e| {
                    let mut lp = get_bone_with_fallback(e, target_bone.to_body_part().to_name())?;
                    if config.aimbot.ground_offset_enabled {
                        let root_y = e.root_position().map(|p| p.y).unwrap_or(lp.y);
                        if root_y <= 3.0 { lp.y += config.aimbot.ground_offset_y; }
                    }
                    let ls = self.visengine.world_to_screen(lp, dims, &view_matrix)?;
                    let k = if e.humanoid_address != 0 { e.humanoid_address } else { e.model_address };
                    Some((k, lp, ls, e.name.clone()))
                })
        } else {
            None
        };

        if self.locked_target_addr != 0 {
            if let Some((addr, _, screen, name)) = &result {
                if *addr == self.locked_target_addr {
                    let dist = screen.distance_to(screen_center);
                    if dist < 600.0 {
                        self.lock_break_frames = 0;
                        return result;
                    }
                }
                let locked_still_valid = ctx.snapshot.iter().any(|e| {
                    let k = if e.humanoid_address != 0 { e.humanoid_address } else { e.model_address };
                    k == self.locked_target_addr && !e.is_dead()
                });
                if locked_still_valid {
                    self.lock_break_frames = self.lock_break_frames.saturating_add(1);
                    if self.lock_break_frames < 6 {
                        if let Some(locked_data) = locked_entity_current {
                            return Some(locked_data);
                        }
                    } else {
                        self.locked_target_addr = *addr;
                        self.locked_target_name = Some(name.clone());
                        self.lock_break_frames = 0;
                    }
                } else {
                    self.locked_target_addr = *addr;
                    self.locked_target_name = Some(name.clone());
                    self.lock_break_frames = 0;
                }
                return result;
            } else {
                self.lock_break_frames = self.lock_break_frames.saturating_add(1);
                if self.lock_break_frames >= 10 {
                    self.locked_target_addr = 0;
                    self.locked_target_name = None;
                    self.lock_break_frames = 0;
                }
                return None;
            }
        }

        if let Some((addr, _, _, name)) = &result {
            self.locked_target_addr = *addr;
            self.locked_target_name = Some(name.clone());
            self.lock_break_frames = 0;
        }
        result
    }

    fn predict_position(&mut self, target_addr: u64, current_pos: Vector3, config: &Config) -> Vector3 {
        if !config.aimbot.prediction_enabled { return current_pos; }
        let tracker = self.velocity_trackers
            .entry(target_addr)
            .or_insert_with(|| VelocityTracker::new(current_pos));
        let (velocity, _) = tracker.update(current_pos);
        if velocity.length() < VELOCITY_THRESHOLD { return current_pos; }
        let time_ahead = config.aimbot.prediction_amount;
        tracker.predict(current_pos, time_ahead)
    }

    pub fn update(&mut self, config: &Config, local_player_name: &str, camera_addr: u64, forced_target: Option<&str>) {
        crate::perf_scope!("viewport_aim_update");

        self.tracker_prune_counter = self.tracker_prune_counter.wrapping_add(1);
        if self.tracker_prune_counter % 300 == 0 {
            self.velocity_trackers.retain(|_, t| t.elapsed_secs() < 60.0);
        }

        if !config.viewport_aim.enabled {
            if self.is_spoofing { self.reset_viewport(); }
            self.current_target_name = None;
            return;
        }

        if !is_valid_address(camera_addr) {
            if self.is_spoofing { self.reset_viewport(); }
            return;
        }

        self.camera_addr = camera_addr;
        let mouse_down = Input::is_key_down(0x01);

        if !mouse_down && self.is_spoofing {
            self.reset_viewport();
            self.last_mouse_state = false;
            return;
        }

        if !mouse_down {
            self.last_mouse_state = false;
            return;
        }

        // Grenade check skipped in demo

        let dims = self.visengine.get_dimensions();
        if dims.x <= 0.0 || dims.y <= 0.0 { return; }

        let screen_center = Vector2::new(dims.x / 2.0, dims.y / 2.0);
        let camera_pos = self.get_camera_position(camera_addr);
        let view_matrix = self.visengine.get_view_matrix();

        let target = self.find_target(config, camera_pos, screen_center, dims, local_player_name, forced_target);

        let (target_addr, target_pos, _target_screen, target_name) = match target {
            Some(t) => t,
            None => {
                self.current_target_name = None;
                self.reset_viewport();
                return;
            }
        };

        self.current_target_name = Some(target_name);

        let predicted_pos = self.predict_position(target_addr, target_pos, config);

        let predicted_screen = match self.visengine.world_to_screen(predicted_pos, dims, &view_matrix) {
            Some(pos) => pos,
            None => { self.reset_viewport(); return; }
        };

        let spoofed_viewport = Vector2int16::calculate_for_target(predicted_screen, dims);
        self.write_viewport(camera_addr, spoofed_viewport);
        self.is_spoofing = true;
        self.last_target_addr = target_addr;

        let shot_initiated = mouse_down && !self.last_mouse_state;
        if shot_initiated { self.shot_count += 1; }
        self.last_mouse_state = mouse_down;
    }

    pub fn get_current_target_name(&self) -> Option<&str> {
        self.current_target_name.as_deref()
    }
}
