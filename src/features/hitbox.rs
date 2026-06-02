// Hitbox Expander feature (PREMIUM VERSION ONLY)

use std::sync::Arc;
use crate::config::HitboxConfig;
use crate::core::memory::Memory;
use crate::utils::cache::Cache;

pub struct HitboxExpander;

impl HitboxExpander {
    pub fn new(_memory: Arc<Memory>, _cache: Arc<Cache>, _local_player_name: String) -> Self {
        Self
    }
    
    pub fn update(&mut self, _config: &HitboxConfig) {
        // PREMIUM VERSION ONLY
    }
    
    pub fn disable(&mut self) {
        // PREMIUM VERSION ONLY
    }
}

