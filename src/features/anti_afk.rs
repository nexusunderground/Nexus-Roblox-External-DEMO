// Anti-AFK feature (PREMIUM VERSION ONLY)

use crate::config::AntiAfkConfig;

pub struct AntiAfk;

impl Default for AntiAfk {
    fn default() -> Self {
        Self::new()
    }
}

impl AntiAfk {
    pub fn new() -> Self {
        Self
    }
    
    pub fn update(&mut self, _config: &AntiAfkConfig) {
        // PREMIUM VERSION ONLY
    }
}

