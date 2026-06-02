#![allow(dead_code)]

use std::sync::Arc;
use crate::config::Config;
use crate::core::memory::Memory;
use crate::core::offsets::camera;
use crate::sdk::Instance;

pub struct WorldModifier {
    memory: Arc<Memory>,
    camera_address: u64,
    original_fov: Option<f32>,
    fov_was_active: bool,
}

impl WorldModifier {
    pub fn new(memory: Arc<Memory>, datamodel: &Arc<Instance>) -> Self {
        let camera_address = datamodel
            .find_first_child_by_class("Workspace")
            .and_then(|ws| ws.find_first_child("Camera"))
            .map(|cam| cam.address)
            .unwrap_or(0);
        
        Self {
            memory,
            camera_address,
            original_fov: None,
            fov_was_active: false,
        }
    }
    
    pub fn update(&mut self, _config: &Config) {}
    
    pub fn apply_all(&mut self, config: &Config) {
        self.apply_fov_changer(config);
    }
    
    fn apply_fov_changer(&mut self, config: &Config) {
        if self.camera_address == 0 {
            return;
        }
        
        if config.camera.fov_enabled {
            if self.original_fov.is_none() {
                let current_fov = self.memory.read::<f32>(self.camera_address + camera::field_of_view());
                if current_fov > 0.0 && current_fov < 180.0 {
                    self.original_fov = Some(current_fov);
                }
            }
            
            let target_fov = config.camera.fov_value.clamp(30.0, 120.0);
            self.memory.write::<f32>(self.camera_address + camera::field_of_view(), target_fov);
            self.fov_was_active = true;
        } else if self.fov_was_active {
            if let Some(original) = self.original_fov {
                self.memory.write::<f32>(self.camera_address + camera::field_of_view(), original);
            }
            self.fov_was_active = false;
            self.original_fov = None;
        }
    }
}

