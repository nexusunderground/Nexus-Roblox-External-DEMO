//! ESP Render Cache - Background thread for pre-computing ESP data.
//!
//! Simplified version for the Nexus demo (no game-specific features).

use ahash::AHashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache};
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::is_teammate;

// LOD distance thresholds
const LOD_HIGH_DISTANCE: f32 = 50.0;
const LOD_MEDIUM_DISTANCE: f32 = 150.0;

// Update intervals
const LOD_HIGH_INTERVAL_MS: u64 = 8;
const LOD_MEDIUM_INTERVAL_MS: u64 = 16;
const LOD_LOW_INTERVAL_MS: u64 = 32;

const MAX_RENDER_ENTITIES: usize = 64;

/// Level of Detail for update frequency
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LodLevel {
    High,
    Medium,
    Low,
}

impl LodLevel {
    pub fn from_distance(distance: f32) -> Self {
        if distance < LOD_HIGH_DISTANCE {
            LodLevel::High
        } else if distance < LOD_MEDIUM_DISTANCE {
            LodLevel::Medium
        } else {
            LodLevel::Low
        }
    }
}

/// Pre-computed ESP data for a single entity.
#[derive(Clone)]
pub struct EspRenderData {
    pub entity_key: u64,
    pub name: Arc<str>,
    pub distance: f32,
    pub is_aim_target: bool,
    pub health_percent: f32,
    pub armor_percent: f32,
    pub has_armor: bool,
    pub is_teammate: bool,
    pub is_visible: bool,
    pub lod_level: LodLevel,
    pub role_tag: Arc<str>,
    pub equipped_weapon: Arc<str>,
    pub world_pos: Vector3,
    pub world_bottom: Vector3,
    pub world_top: Vector3,
    pub box_3d_corners_world: Option<[Vector3; 8]>,
    pub computed_at: Instant,
    pub is_game_specific: bool,
}

/// Thread-safe ESP render cache.
pub struct EspRenderCache {
    render_data: RwLock<Arc<Vec<EspRenderData>>>,
    config_snapshot: RwLock<CachedConfig>,
    local_player_name: RwLock<String>,
    aim_target_name: RwLock<String>,
    running: AtomicBool,
}

#[derive(Clone, Default)]
struct CachedConfig {
    max_distance: f32,
    team_check: bool,
    hide_dead: bool,
    teammate_whitelist: Vec<String>,
}

impl EspRenderCache {
    pub fn new() -> Self {
        Self {
            render_data: RwLock::new(Arc::new(Vec::new())),
            config_snapshot: RwLock::new(CachedConfig::default()),
            local_player_name: RwLock::new(String::new()),
            aim_target_name: RwLock::new(String::new()),
            running: AtomicBool::new(false),
        }
    }

    /// Get the current render data snapshot (lock-free Arc clone).
    pub fn get_render_data(&self) -> Arc<Vec<EspRenderData>> {
        self.render_data.read().map(|g| Arc::clone(&*g)).unwrap_or_default()
    }

    /// Update the config snapshot (called from render thread each frame).
    pub fn update_config(&self, config: &Config, local_name: &str, aim_target: &str) {
        if let Ok(mut snap) = self.config_snapshot.write() {
            snap.max_distance = config.visuals.max_distance;
            snap.team_check = config.visuals.team_check;
            snap.hide_dead = config.visuals.hide_dead;
            snap.teammate_whitelist = config.visuals.teammate_whitelist.clone();
        }
        if let Ok(mut name) = self.local_player_name.write() {
            *name = local_name.to_string();
        }
        if let Ok(mut aim) = self.aim_target_name.write() {
            *aim = aim_target.to_string();
        }
    }

    /// Start the background data thread.
    pub fn start(
        self_arc: Arc<Self>,
        cache: Arc<Cache>,
        visengine: Arc<VisualEngine>,
    ) {
        if self_arc.running.swap(true, Ordering::SeqCst) {
            return;
        }

        thread::spawn(move || {
            while self_arc.running.load(Ordering::Relaxed) {
                let start = Instant::now();

                let config_snap = self_arc.config_snapshot.read()
                    .map(|g| g.clone())
                    .unwrap_or_default();
                let local_name = self_arc.local_player_name.read()
                    .map(|g| g.clone())
                    .unwrap_or_default();
                let aim_target = self_arc.aim_target_name.read()
                    .map(|g| g.clone())
                    .unwrap_or_default();

                let snapshot = cache.get_snapshot();
                let view_matrix = visengine.get_view_matrix();
                let dimensions = visengine.get_dimensions();
                let window_offset = visengine.get_window_offset();
                let local_team = cache.get_local_team_addr();

                let teammate_addresses: AHashSet<u64> = if config_snap.team_check
                    && !config_snap.teammate_whitelist.is_empty()
                {
                    snapshot.iter()
                        .filter(|e| config_snap.teammate_whitelist.iter()
                            .any(|n| n.eq_ignore_ascii_case(&e.name)))
                        .map(|e| e.model_address)
                        .collect()
                } else {
                    AHashSet::new()
                };

                // Estimate local position from first non-local entity (approximation) 
                // or fall back to camera position
                let local_pos = visengine.get_camera_position()
                    .unwrap_or(Vector3::ZERO);

                let mut render_list: Vec<EspRenderData> = snapshot.iter()
                    .filter(|entity| {
                        if entity.name.eq_ignore_ascii_case(&local_name) { return false; }
                        if config_snap.hide_dead && entity.is_dead() { return false; }
                        true
                    })
                    .filter_map(|entity| {
                        let root_pos = entity.root_position().unwrap_or(Vector3::ZERO);
                        if !root_pos.is_valid() { return None; }

                        let distance = local_pos.distance_to(root_pos);
                        if distance > config_snap.max_distance { return None; }

                        let lod = LodLevel::from_distance(distance);

                        let team = is_teammate(entity, config_snap.team_check, local_team, &teammate_addresses);

                        let health_percent = if entity.max_health > 0.0 {
                            entity.health / entity.max_health
                        } else { 0.0 };
                        let armor_percent = if entity.max_armor > 0.0 {
                            entity.armor / entity.max_armor
                        } else { 0.0 };

                        // Compute world-space box corners from head to feet
                        let head_pos = entity.parts.get(&BodyPart::Head)
                            .map(|p| p.position)
                            .unwrap_or(root_pos + Vector3::new(0.0, 2.8, 0.0));
                        let foot_pos = root_pos - Vector3::new(0.0, 2.8, 0.0);

                        Some(EspRenderData {
                            entity_key: entity.model_address,
                            name: Arc::from(entity.name.as_str()),
                            distance,
                            is_aim_target: entity.name == aim_target,
                            health_percent,
                            armor_percent,
                            has_armor: entity.armor > 0.0,
                            is_teammate: team,
                            is_visible: true,
                            lod_level: lod,
                            role_tag: Arc::from(""),
                            equipped_weapon: Arc::from(entity.equipped_weapon.as_str()),
                            world_pos: root_pos,
                            world_bottom: foot_pos,
                            world_top: head_pos + Vector3::new(0.0, 0.3, 0.0),
                            box_3d_corners_world: None,
                            computed_at: Instant::now(),
                            is_game_specific: entity.is_game_specific,
                        })
                    })
                    .collect();

                render_list.sort_by(|a, b| b.distance.partial_cmp(&a.distance).unwrap_or(std::cmp::Ordering::Equal));
                render_list.truncate(MAX_RENDER_ENTITIES);

                if let Ok(mut guard) = self_arc.render_data.write() {
                    *guard = Arc::new(render_list);
                }

                let elapsed = start.elapsed();
                let target = Duration::from_millis(LOD_HIGH_INTERVAL_MS);
                if elapsed < target {
                    thread::sleep(target - elapsed);
                }
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for EspRenderCache {
    fn default() -> Self {
        Self::new()
    }
}
