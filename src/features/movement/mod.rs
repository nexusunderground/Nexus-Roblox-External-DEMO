#![allow(dead_code)]
#![allow(unused_imports)]

use std::sync::Arc;
use std::time::Instant;
use crate::config::Config;
use crate::core::memory::{Memory, is_valid_address};
use crate::core::offsets::{humanoid, base_part};
use crate::sdk::Player;
use crate::utils::cache::{Cache, BodyPart};

#[derive(Clone, Debug)]
pub enum OffsetPattern {
    BoolToggle,
    StateEnum,
    LargeInt,
    NullPattern,
}

#[derive(Clone)]
pub struct DiscoveredOffset {
    pub offset: u64,
    pub name: String,
    pub data_type: String,
    pub value: String,
    pub label: Option<String>,
    pub cluster_size: Option<usize>,
    pub changes: Vec<String>,
    pub pattern: Option<OffsetPattern>,
}

pub struct MovementHacks {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    local_player_name: String,
    original_walkspeed: Option<f32>,
    original_jump_power: Option<f32>,
    last_jump_time: Instant,
    spinbot_angle: f32,
    spinbot_last_time: Instant,
    speed_was_active: bool,
    jump_power_was_active: bool,
}

impl MovementHacks {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, local_player_name: String) -> Self {
        Self {
            memory,
            cache,
            local_player_name,
            original_walkspeed: None,
            original_jump_power: None,
            last_jump_time: Instant::now(),
            spinbot_angle: 0.0,
            spinbot_last_time: Instant::now(),
            speed_was_active: false,
            jump_power_was_active: false,
        }
    }
    
    fn get_local_humanoid(&self) -> Option<u64> {
        let entities = self.cache.get_snapshot();
        entities.iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
            .map(|e| e.humanoid_address)
            .filter(|&addr| addr != 0)
    }
    
    fn get_root_part_address(&self) -> Option<u64> {
        self.cache
            .get_snapshot()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
            .and_then(|e| e.parts.get(&BodyPart::HumanoidRootPart))
            .map(|p| p.address)
    }
    
    pub fn update(&mut self, _config: &Config) {}
    
    pub fn apply_all(&mut self, config: &Config, menu_open: bool) {
        if menu_open {
            return;
        }
        
        self.apply_walkspeed(config);
        self.apply_jump_power(config);
        self.apply_auto_jump(config);
        self.apply_spinbot(config);
    }
    
    fn apply_walkspeed(&mut self, config: &Config) {
        let target_speed = config.movement.walk_speed;
        
        if target_speed <= 16.0 {
            if self.speed_was_active {
                if let Some(humanoid_addr) = self.get_local_humanoid() {
                    if let Some(original) = self.original_walkspeed {
                        self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed(), original);
                        self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed_check(), original);
                    }
                }
                self.speed_was_active = false;
                self.original_walkspeed = None;
            }
            return;
        }

        if let Some(humanoid_addr) = self.get_local_humanoid() {
            if self.original_walkspeed.is_none() {
                let current = self.memory.read::<f32>(humanoid_addr + humanoid::walkspeed());
                if current > 0.0 && current < 1000.0 {
                    self.original_walkspeed = Some(current);
                }
            }
            self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed(), target_speed);
            self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed_check(), target_speed);
            self.speed_was_active = true;
        }
    }
    
    fn apply_jump_power(&mut self, config: &Config) {
        let target_power = config.movement.jump_power;
        
        if target_power <= 50.0 {
            if self.jump_power_was_active {
                if let Some(humanoid_addr) = self.get_local_humanoid() {
                    if let Some(original) = self.original_jump_power {
                        self.memory.write::<f32>(humanoid_addr + humanoid::jump_power(), original);
                        self.memory.write::<f32>(humanoid_addr + humanoid::jump_height(), original * 0.15);
                    }
                }
                self.jump_power_was_active = false;
                self.original_jump_power = None;
            }
            return;
        }

        if let Some(humanoid_addr) = self.get_local_humanoid() {
            if self.original_jump_power.is_none() {
                let current = self.memory.read::<f32>(humanoid_addr + humanoid::jump_power());
                if current > 0.0 && current < 1000.0 {
                    self.original_jump_power = Some(current);
                }
            }
            self.memory.write::<f32>(humanoid_addr + humanoid::jump_power(), target_power);
            self.memory.write::<f32>(humanoid_addr + humanoid::jump_height(), target_power * 0.15);
            self.jump_power_was_active = true;
        }
    }
    
    fn apply_auto_jump(&mut self, config: &Config) {
        if !config.movement.auto_jump {
            return;
        }

        let humanoid_addr = match self.get_local_humanoid() {
            Some(addr) => addr,
            None => return,
        };

        let floor_material = self.memory.read::<i32>(humanoid_addr + humanoid::floor_material());
        let on_ground = floor_material != 0;

        if on_ground {
            self.memory.write::<bool>(humanoid_addr + humanoid::jump(), true);
        }
    }
    
    fn apply_spinbot(&mut self, config: &Config) {
        if !config.movement.spinbot_enabled {
            if let Some(humanoid_addr) = self.get_local_humanoid() {
                self.memory.write::<u8>(humanoid_addr + humanoid::auto_rotate(), 1);
            }
            self.spinbot_angle = 0.0;
            self.spinbot_last_time = Instant::now();
            return;
        }

        let humanoid_addr = match self.get_local_humanoid() {
            Some(addr) => addr,
            None => return,
        };

        let hrp = match self.get_root_part_address() {
            Some(addr) if is_valid_address(addr) => addr,
            _ => return,
        };

        let prim = self.memory.read::<u64>(hrp + base_part::primitive());
        if !is_valid_address(prim) {
            return;
        }

        self.memory.write::<u8>(humanoid_addr + humanoid::auto_rotate(), 0);

        let now = Instant::now();
        let delta_time = now.duration_since(self.spinbot_last_time).as_secs_f32();
        self.spinbot_last_time = now;
        
        let delta_time = delta_time.min(0.1);

        let degrees_per_second = config.movement.spinbot_speed * 60.0;
        let radians_per_second = degrees_per_second * (std::f32::consts::PI / 180.0);
        
        self.spinbot_angle += radians_per_second * delta_time;
        
        while self.spinbot_angle >= std::f32::consts::TAU {
            self.spinbot_angle -= std::f32::consts::TAU;
        }
        while self.spinbot_angle < 0.0 {
            self.spinbot_angle += std::f32::consts::TAU;
        }

        let cos_a = self.spinbot_angle.cos();
        let sin_a = self.spinbot_angle.sin();
        
        let rotation_matrix: [f32; 9] = [
            cos_a,  0.0,  sin_a,
            0.0,    1.0,  0.0,
            -sin_a, 0.0,  cos_a,
        ];
        
        let rotation_offset = base_part::rotation();
        self.memory.write::<[f32; 9]>(prim + rotation_offset, rotation_matrix);
    }
    
    pub fn handle_hotkeys(&mut self, _config: &mut Config) {}
    
    pub fn get_offset_snapshot(&self) -> Vec<DiscoveredOffset> {
        Vec::new()
    }
    
    pub fn get_candidates(&self) -> Vec<DiscoveredOffset> {
        Vec::new()
    }
    
    pub fn clear_candidates(&mut self) {}
    
    pub fn debug_scan_mouse_service(&mut self) {}
}

impl Drop for MovementHacks {
    fn drop(&mut self) {
        if let Some(humanoid_addr) = self.get_local_humanoid() {
            if let Some(original_speed) = self.original_walkspeed {
                self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed(), original_speed);
                self.memory.write::<f32>(humanoid_addr + humanoid::walkspeed_check(), original_speed);
            }
            if let Some(original_power) = self.original_jump_power {
                self.memory.write::<f32>(humanoid_addr + humanoid::jump_power(), original_power);
                self.memory.write::<f32>(humanoid_addr + humanoid::jump_height(), original_power * 0.15);
            }
            self.memory.write::<u8>(humanoid_addr + humanoid::auto_rotate(), 1);
        }
    }
}
 