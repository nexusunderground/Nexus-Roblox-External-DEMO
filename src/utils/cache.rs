#![allow(dead_code)]

use ahash::AHashSet;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use rayon::prelude::*;
use smallvec::SmallVec;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::humanoid;
use crate::sdk::{Humanoid, Instance, Part, Player};
use crate::utils::math::{Matrix3, Vector3};

const DEFAULT_UPDATE_INTERVAL_MS: u64 = 20; // ~50Hz base rate
const HIGH_PRIORITY_INTERVAL_MS: u64 = 4;   // ~250Hz when aim key held
const VELOCITY_SAMPLES: usize = 8;          // More samples for ultra-smooth velocity
const VELOCITY_SMOOTHING: f32 = 0.85;       // Higher = more responsive to velocity changes

pub const DEAD_HEALTH_THRESHOLD: f32 = 1.0;
const STALE_MODEL_THRESHOLD: usize = 5;
const CACHE_CLEAR_COOLDOWN_SECS: u64 = 3;
const POSITION_STALE_CHECK_CYCLES: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPart {
    Head,
    Torso,
    HumanoidRootPart,
    UpperTorso,
    LowerTorso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
    /// Accessory handles (hair, hats, face, back, etc.)
    Accessory(u8),
}

impl BodyPart {
    #[inline]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Head" => Some(Self::Head),
            "Torso" => Some(Self::Torso),
            "HumanoidRootPart" => Some(Self::HumanoidRootPart),
            "UpperTorso" => Some(Self::UpperTorso),
            "LowerTorso" => Some(Self::LowerTorso),
            "Left Arm" => Some(Self::LeftArm),
            "Right Arm" => Some(Self::RightArm),
            "Left Leg" => Some(Self::LeftLeg),
            "Right Leg" => Some(Self::RightLeg),
            "LeftUpperArm" => Some(Self::LeftUpperArm),
            "LeftLowerArm" => Some(Self::LeftLowerArm),
            "LeftHand" => Some(Self::LeftHand),
            "RightUpperArm" => Some(Self::RightUpperArm),
            "RightLowerArm" => Some(Self::RightLowerArm),
            "RightHand" => Some(Self::RightHand),
            "LeftUpperLeg" => Some(Self::LeftUpperLeg),
            "LeftLowerLeg" => Some(Self::LeftLowerLeg),
            "LeftFoot" => Some(Self::LeftFoot),
            "RightUpperLeg" => Some(Self::RightUpperLeg),
            "RightLowerLeg" => Some(Self::RightLowerLeg),
            "RightFoot" => Some(Self::RightFoot),
            _ => None,
        }
    }

    /// Whether this body part is an accessory (hair, hat, etc.).
    #[inline]
    pub const fn is_accessory(self) -> bool {
        matches!(self, Self::Accessory(_))
    }
    
    #[inline]
    pub const fn to_name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Torso => "Torso",
            Self::HumanoidRootPart => "HumanoidRootPart",
            Self::UpperTorso => "UpperTorso",
            Self::LowerTorso => "LowerTorso",
            Self::LeftArm => "Left Arm",
            Self::RightArm => "Right Arm",
            Self::LeftLeg => "Left Leg",
            Self::RightLeg => "Right Leg",
            Self::LeftUpperArm => "LeftUpperArm",
            Self::LeftLowerArm => "LeftLowerArm",
            Self::LeftHand => "LeftHand",
            Self::RightUpperArm => "RightUpperArm",
            Self::RightLowerArm => "RightLowerArm",
            Self::RightHand => "RightHand",
            Self::LeftUpperLeg => "LeftUpperLeg",
            Self::LeftLowerLeg => "LeftLowerLeg",
            Self::LeftFoot => "LeftFoot",
            Self::RightUpperLeg => "RightUpperLeg",
            Self::RightLowerLeg => "RightLowerLeg",
            Self::RightFoot => "RightFoot",
            Self::Accessory(_) => "Accessory",
        }
    }
    
    pub const ALL: [BodyPart; 21] = [
        Self::Head, Self::Torso, Self::HumanoidRootPart,
        Self::UpperTorso, Self::LowerTorso,
        Self::LeftArm, Self::RightArm, Self::LeftLeg, Self::RightLeg,
        Self::LeftUpperArm, Self::LeftLowerArm, Self::LeftHand,
        Self::RightUpperArm, Self::RightLowerArm, Self::RightHand,
        Self::LeftUpperLeg, Self::LeftLowerLeg, Self::LeftFoot,
        Self::RightUpperLeg, Self::RightLowerLeg, Self::RightFoot,
    ];

    /// Visible body parts for R15 rigs (excludes HumanoidRootPart which is invisible).
    pub const VISIBLE_R15: &'static [BodyPart] = &[
        Self::Head,
        Self::UpperTorso, Self::LowerTorso,
        Self::LeftUpperArm, Self::RightUpperArm,
        Self::LeftLowerArm, Self::RightLowerArm,
        Self::LeftHand, Self::RightHand,
        Self::LeftUpperLeg, Self::RightUpperLeg,
        Self::LeftLowerLeg, Self::RightLowerLeg,
        Self::LeftFoot, Self::RightFoot,
    ];

    /// Visible body parts for R6 rigs.
    pub const VISIBLE_R6: &'static [BodyPart] = &[
        Self::Head, Self::Torso,
        Self::LeftArm, Self::RightArm,
        Self::LeftLeg, Self::RightLeg,
    ];
}

#[derive(Clone)]
pub struct Entity {
    pub model_address: u64,
    pub name: String,
    pub rig_type: u8,
    pub humanoid_address: u64,
    pub team_address: u64,
    pub team_identifier: String,
    pub body_effects_address: u64,
    pub armor_value_address: u64,
    pub equipped_tool_sv_address: u64,
    pub parts: ahash::AHashMap<BodyPart, PartData>,
    pub velocity: Vector3,
    pub health: f32,
    pub max_health: f32,
    pub armor: f32,
    pub max_armor: f32,
    pub has_teammate_label: bool,
    pub is_game_specific: bool,
    pub is_transparent: bool,
    pub account_age: i32,
    pub equipped_weapon: String,
}

impl Entity {
    #[inline]
    pub fn is_dead(&self) -> bool {
        self.health < DEAD_HEALTH_THRESHOLD || self.health.is_nan() || self.health.is_infinite()
    }

    /// Get the root body part data: HRP → UpperTorso → Torso → Head.
    #[inline]
    pub fn root_part(&self) -> Option<&PartData> {
        self.parts.get(&BodyPart::HumanoidRootPart)
            .or_else(|| self.parts.get(&BodyPart::UpperTorso))
            .or_else(|| self.parts.get(&BodyPart::Torso))
            .or_else(|| self.parts.get(&BodyPart::Head))
    }

    /// Convenience: get the root part's world position.
    #[inline]
    pub fn root_position(&self) -> Option<Vector3> {
        self.root_part().map(|p| p.position)
    }
}

#[derive(Clone)]
pub struct PartData {
    pub address: u64,
    pub primitive_address: u64,
    pub size: Vector3,
    pub position: Vector3,
    pub rotation: Matrix3,
}

#[derive(Clone)]
struct PositionSample {
    position: Vector3,
    timestamp: Instant,
}

type PositionHistoryVec = SmallVec<[PositionSample; VELOCITY_SAMPLES]>;

pub struct Cache {
    cached_players: Arc<DashMap<u64, Entity>>,
    position_history: Arc<DashMap<u64, PositionHistoryVec>>,
    smoothed_velocities: Arc<DashMap<u64, Vector3>>,
    snapshot: Arc<ArcSwap<Vec<Entity>>>,
    update_interval_ms: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    show_bots: Arc<AtomicBool>,
    high_priority: Arc<AtomicBool>,
    local_team_addr: Arc<AtomicU64>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cached_players: Arc::new(DashMap::new()),
            position_history: Arc::new(DashMap::new()),
            smoothed_velocities: Arc::new(DashMap::new()),
            snapshot: Arc::new(ArcSwap::from_pointee(Vec::new())),
            update_interval_ms: Arc::new(AtomicU64::new(DEFAULT_UPDATE_INTERVAL_MS)),
            running: Arc::new(AtomicBool::new(false)),
            show_bots: Arc::new(AtomicBool::new(false)),
            high_priority: Arc::new(AtomicBool::new(false)),
            local_team_addr: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_high_priority(&self, enabled: bool) {
        self.high_priority.store(enabled, Ordering::Relaxed);
    }
    pub fn set_show_bots(&self, enabled: bool) {
        self.show_bots.store(enabled, Ordering::Relaxed);
    }
    pub fn set_update_rate(&self, ms: u64) {
        self.update_interval_ms.store(ms.max(16).min(200), Ordering::Relaxed);
    }
    pub fn clear(&self) {
        self.cached_players.clear();
        self.position_history.clear();
        self.smoothed_velocities.clear();
        self.snapshot.store(Arc::new(Vec::new()));
    }
    pub fn count(&self) -> usize {
        self.cached_players.len()
    }

    pub fn get_snapshot(&self) -> Arc<Vec<Entity>> {
        self.snapshot.load_full()
    }

    /// Returns the local player's team address (0 if not found).
    pub fn get_local_team_addr(&self) -> u64 {
        self.local_team_addr.load(Ordering::Relaxed)
    }

    /// Returns the local player's team identifier string.
    /// (Not used in the demo — game-specific only. Returns empty string.)
    pub fn get_local_team_id(&self) -> String {
        String::new()
    }

    pub fn get_local_team_address(&self, local_player_name: &str) -> u64 {
        self.cached_players
            .iter()
            .find(|r| r.value().name.eq_ignore_ascii_case(local_player_name))
            .map(|r| r.value().team_address)
            .unwrap_or(0)
    }


    pub fn start(&self, players_instance: Arc<Instance>, workspace_instance: Arc<Instance>, memory: Arc<Memory>, update_rate_ms: u64) {
        if self.running.swap(true, Ordering::SeqCst) {
            tracing::warn!("Cache thread already running");
            return;
        }

        self.update_interval_ms.store(update_rate_ms, Ordering::Relaxed);

        let cached_players = Arc::clone(&self.cached_players);
        let position_history = Arc::clone(&self.position_history);
        let smoothed_velocities = Arc::clone(&self.smoothed_velocities);
        let snapshot = Arc::clone(&self.snapshot);
        let local_team_addr = Arc::clone(&self.local_team_addr);
        let update_interval_ms = Arc::clone(&self.update_interval_ms);
        let running = Arc::clone(&self.running);
        let show_bots = Arc::clone(&self.show_bots);
        let high_priority = Arc::clone(&self.high_priority);

        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let bots_enabled = show_bots.load(Ordering::Relaxed);
                Self::update_cache(&cached_players, &position_history, &smoothed_velocities, &snapshot, &local_team_addr, &players_instance, &workspace_instance, &memory, bots_enabled);
                
                let interval = if high_priority.load(Ordering::Relaxed) {
                    HIGH_PRIORITY_INTERVAL_MS
                } else {
                    update_interval_ms.load(Ordering::Relaxed)
                };
                thread::sleep(Duration::from_millis(interval));
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn restart(&self, players_instance: Arc<Instance>, workspace_instance: Arc<Instance>, memory: Arc<Memory>, update_rate_ms: u64) {
        self.stop();
        
        std::thread::sleep(Duration::from_millis(100));
        
        self.clear();
        
        self.running.store(false, Ordering::SeqCst);
        self.start(players_instance, workspace_instance, memory, update_rate_ms);
    }

    fn update_cache(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
        snapshot: &Arc<ArcSwap<Vec<Entity>>>,
        local_team_addr: &Arc<AtomicU64>,
        players_instance: &Arc<Instance>,
        workspace_instance: &Arc<Instance>,
        memory: &Arc<Memory>,
        show_bots: bool,
    ) {
        let children = players_instance.get_children();
        let now = Instant::now();

        let player_children: Vec<_> = children
            .into_iter()
            .filter(|child| child.get_class_name() == "Player")
            .collect();

        static LAST_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last = LAST_LOG.load(std::sync::atomic::Ordering::Relaxed);
        if current_time - last >= 5 {
            tracing::trace!("Cache: Found {} players", player_children.len());
            LAST_LOG.store(current_time, std::sync::atomic::Ordering::Relaxed);
        }

        let mut current_addresses: AHashSet<u64> = player_children.iter().map(|c| c.address).collect();

        let player_model_info: Vec<(u64, String)> = player_children
            .iter()
            .filter_map(|child| {
                let player = Player::new(child.address, Arc::clone(memory));
                let model = player.get_model_instance();
                let model_addr = model.address();
                let name = player.get_name();
                if is_valid_address(model_addr) && !name.is_empty() {
                    Some((model_addr, name))
                } else {
                    None
                }
            })
            .collect();
        
        let player_model_addresses: AHashSet<u64> = player_model_info.iter().map(|(addr, _)| *addr).collect();
        let player_names: AHashSet<String> = player_model_info.iter().map(|(_, name)| name.clone()).collect();

        let updates: Vec<(u64, Option<Entity>, Option<(u64, Vector3)>, bool)> = player_children
            .par_iter()
            .filter_map(|child| {
                let player = Player::new(child.address, Arc::clone(memory));
                let model = player.get_model_instance();
                let model_addr = model.address();

                if !is_valid_address(model_addr) {
                    let was_cached = cached_players.contains_key(&child.address);
                    return Some((child.address, None, None, was_cached));
                }

                let existing_check = cached_players
                    .get(&child.address)
                    .map(|r| (r.model_address == model_addr, r.clone()));

                if let Some((same_model, mut existing)) = existing_check {
                    if same_model {
                        let is_valid = Self::update_entity_positions(&mut existing, &model, memory);
                        if !is_valid {
                            return Some((child.address, None, None, true));
                        }
                        existing.team_address = player.get_team_address();
                        let root_pos = existing.parts.get(&BodyPart::HumanoidRootPart).map(|p| p.position);
                        return Some((child.address, Some(existing), root_pos.map(|p| (child.address, p)), false));
                    } else {
                        if let Some(entity) = Self::build_entity(&player, &model, memory) {
                            let root_pos = entity.parts.get(&BodyPart::HumanoidRootPart).map(|p| p.position);
                            return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p)), false));
                        } else {
                            return Some((child.address, None, None, true));
                        }
                    }
                }

                if let Some(entity) = Self::build_entity(&player, &model, memory) {
                    let root_pos = entity.parts.get(&BodyPart::HumanoidRootPart).map(|p| p.position);
                    return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p)), false));
                }

                None
            })
            .collect();

        let stale_count = updates.iter().filter(|(_, _, _, is_stale)| *is_stale).count();
        
        static LAST_CACHE_CLEAR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let current_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last_clear = LAST_CACHE_CLEAR.load(std::sync::atomic::Ordering::Relaxed);
        
         if stale_count >= STALE_MODEL_THRESHOLD && (current_secs - last_clear) >= CACHE_CLEAR_COOLDOWN_SECS {
            tracing::info!("Round transition detected ({} stale entities) - selective cache cleanup", stale_count);
            // Selective cleanup: only remove entities that are truly stale (no valid player anymore)
            // Keep position_history and smoothed_velocities for smoother re-acquisition
            let stale_addresses: Vec<u64> = cached_players
                .iter()
                .filter(|entry| {
                    let entity = entry.value();
                    // Entity is stale if humanoid is invalid or all parts are at origin
                    !is_valid_address(entity.humanoid_address) || 
                    entity.parts.values().all(|p| p.position.is_near_origin(5.0))
                })
                .map(|entry| *entry.key())
                .collect();
            
            for addr in stale_addresses {
                cached_players.remove(&addr);
            }
            LAST_CACHE_CLEAR.store(current_secs, std::sync::atomic::Ordering::Relaxed);
            // Don't return early - continue to rebuild entities
        }

        let npc_updates: Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> = if show_bots {
            Self::scan_workspace_for_npcs(cached_players, workspace_instance, memory, &player_model_addresses, &player_names)
        } else {
            Vec::new()
        };

        for (addr, entity_opt, _) in &npc_updates {
            if entity_opt.is_some() {
                current_addresses.insert(*addr);
            }
        }

        for (address, entity_opt, _, _) in &updates {
            if let Some(entity) = entity_opt {
                cached_players.insert(*address, entity.clone());
            }
        }
        for (address, entity_opt, _) in &npc_updates {
            if let Some(entity) = entity_opt {
                cached_players.insert(*address, entity.clone());
            }
        }

        for (address, entity_opt, root_pos_opt, _) in &updates {
            if let (Some(_), Some((_, root_pos))) = (entity_opt, root_pos_opt) {
                Self::update_velocity_history(position_history, *address, *root_pos, now);
            }
        }
        
        for (address, entity_opt, root_pos_opt) in &npc_updates {
            if let (Some(_), Some((_, root_pos))) = (entity_opt, root_pos_opt) {
                Self::update_velocity_history(position_history, *address, *root_pos, now);
            }
        }

        Self::apply_velocities_from_history(cached_players, position_history, smoothed_velocities);

        Self::cleanup_disconnected(cached_players, position_history, smoothed_velocities, &current_addresses);

        // Publish the snapshot (lock-free atomic swap for readers)
        let new_snap: Vec<Entity> = cached_players.iter().map(|r| r.value().clone()).collect();
        snapshot.store(Arc::new(new_snap));

        // Publish local player's team address (first player whose name isn't known — just use
        // the first player with a non-zero team address; real local-player tracking would need
        // the local player name passed in, but for demo simplicity we store per-entity on read).
        let _ = local_team_addr; // stored per-entity; callers read entity.team_address directly
    }

    fn scan_workspace_for_npcs(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        memory: &Arc<Memory>,
        player_model_addresses: &AHashSet<u64>,
        player_names: &AHashSet<String>,
    ) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
        let mut models_to_check: Vec<Instance> = Vec::new();
        Self::collect_models_recursive(workspace, memory, &mut models_to_check, 3); // Max depth of 3
        
        static LAST_NPC_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last = LAST_NPC_LOG.load(std::sync::atomic::Ordering::Relaxed);
        if current_time - last >= 5 {
            tracing::trace!("NPC scan: Found {} models in workspace", models_to_check.len());
            LAST_NPC_LOG.store(current_time, std::sync::atomic::Ordering::Relaxed);
        }

        models_to_check
            .par_iter()
            .filter_map(|child| {
                if player_model_addresses.contains(&child.address) {
                    return None;
                }

                let model_name = child.get_name();
                if player_names.contains(&model_name) {
                    return None;
                }

                let model = crate::sdk::ModelInstance::new(child.address, Arc::clone(memory));
                let humanoid = model.find_first_child("Humanoid")?;
                
                if humanoid.get_class_name() != "Humanoid" {
                    return None;
                }

                let model_addr = model.address();
                if !is_valid_address(model_addr) {
                    return None;
                }

                // Check if existing NPC needs position update only (lock-free with DashMap)
                let existing_check = cached_players
                    .get(&child.address)
                    .map(|r| (r.model_address == model_addr, r.clone()));

                if let Some((same_model, mut existing)) = existing_check {
                    if same_model {
                        let is_valid = Self::update_entity_positions(&mut existing, &model, memory);
                        if !is_valid {
                            return None;
                        }
                        let root_pos = existing.parts.get(&BodyPart::HumanoidRootPart).map(|p| p.position);
                        return Some((child.address, Some(existing), root_pos.map(|p| (child.address, p))));
                    }
                }

                if let Some(entity) = Self::build_npc_entity(&model, memory) {
                    let root_pos = entity.parts.get(&BodyPart::HumanoidRootPart).map(|p| p.position);
                    return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p))));
                }

                None
            })
            .collect()
    }

    fn collect_models_recursive(
        parent: &Instance,
        memory: &Arc<Memory>,
        models: &mut Vec<Instance>,
        max_depth: u32,
    ) {
        if max_depth == 0 {
            return;
        }

        let children = parent.get_children();
        
        for child in children {
            let class_name = child.get_class_name();
            
            match class_name.as_str() {
                "Model" => {
                    models.push(Instance::new(child.address, Arc::clone(memory)));
                }
                "Folder" | "Configuration" => {
                    Self::collect_models_recursive(&child, memory, models, max_depth - 1);
                }
                _ => {
                }
            }
        }
    }

    fn build_npc_entity(
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
    ) -> Option<Entity> {
        let model_children = model.get_children();
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();

        for part_instance in model_children {
            if let Some(body_part) = BodyPart::from_name(&part_instance.get_name()) {
                let part = Part::new(part_instance.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();

                if !position.is_valid() {
                    continue;
                }

                // Cache primitive address for fast position reads (avoids find_first_child every frame)
                parts.insert(
                    body_part,
                    PartData {
                        address: part_instance.address,
                        primitive_address: primitive.address,
                        size: primitive.get_size(),
                        position,
                        rotation: primitive.get_rotation(),
                    },
                );
            }
        }

        if parts.is_empty() {
            return None;
        }

        let (rig_type, humanoid_address) = model
            .find_first_child("Humanoid")
            .map(|h| {
                let hum = Humanoid::new(h.address, Arc::clone(memory));
                (hum.get_rig_type(), h.address)
            })
            .unwrap_or((0, 0));

        let (health, max_health) = if humanoid_address != 0 {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            // Validate health values
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        let model_instance = Instance::new(model.address(), Arc::clone(memory));
        let name = model_instance.get_name();

        let (body_effects_address, armor, max_armor) = model
            .find_first_child("BodyEffects")
            .map(|be| {
                let be_addr = be.address;
                let armor_instance = be.find_first_child("Armor");
                let (armor_val, max_armor_val) = if let Some(armor_inst) = armor_instance {
                    let value = memory.read::<f64>(armor_inst.address + crate::core::offsets::value_base::value());
                    let armor = if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                        value as f32
                    } else {
                        0.0
                    };
                    let max_armor = be.find_first_child("MaxArmor")
                        .map(|ma| {
                            let mv = memory.read::<f64>(ma.address + crate::core::offsets::value_base::value());
                            if !mv.is_nan() && !mv.is_infinite() && mv > 0.0 { mv as f32 } else { 100.0 }
                        })
                        .unwrap_or(100.0);
                    (armor, max_armor)
                } else {
                    (0.0, 0.0)
                };
                (be_addr, armor_val, max_armor_val)
            })
            .unwrap_or((0, 0.0, 0.0));

        Some(Entity {
            model_address: model.address(),
            name,
            rig_type,
            humanoid_address,
            team_address: 0,
            team_identifier: String::new(),
            body_effects_address,
            armor_value_address: 0,
            equipped_tool_sv_address: 0,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor,
            max_armor,
            has_teammate_label: false,
            is_game_specific: false,
            is_transparent: false,
            account_age: 0,
            equipped_weapon: String::new(),
        })
    }

    fn update_velocity_history(
        history: &Arc<DashMap<u64, PositionHistoryVec>>,
        address: u64,
        root_pos: Vector3,
        now: Instant,
    ) {
        let mut samples = history.entry(address).or_insert_with(SmallVec::new);
        samples.push(PositionSample {
            position: root_pos,
            timestamp: now,
        });

        if samples.len() > VELOCITY_SAMPLES {
            let excess = samples.len() - VELOCITY_SAMPLES;
            samples.drain(0..excess);
        }
    }

    fn apply_velocities_from_history(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
    ) {
        for entry in position_history.iter() {
            let address = *entry.key();
            let samples = entry.value();
            
            if samples.len() >= 2 {
                let mut total_velocity = Vector3::ZERO;
                let mut total_weight = 0.0;
                
                for i in 1..samples.len() {
                    let prev = &samples[i - 1];
                    let curr = &samples[i];
                    let dt = curr.timestamp.duration_since(prev.timestamp).as_secs_f32();
                    
                    if dt > 0.001 && dt < 0.5 {
                        let weight = i as f32;
                        let instant_vel = Vector3::new(
                            (curr.position.x - prev.position.x) / dt,
                            (curr.position.y - prev.position.y) / dt,
                            (curr.position.z - prev.position.z) / dt,
                        );
                        total_velocity.x += instant_vel.x * weight;
                        total_velocity.y += instant_vel.y * weight;
                        total_velocity.z += instant_vel.z * weight;
                        total_weight += weight;
                    }
                }
                
                if total_weight > 0.0 {
                    let raw_velocity = Vector3::new(
                        total_velocity.x / total_weight,
                        total_velocity.y / total_weight,
                        total_velocity.z / total_weight,
                    );
                    
                    let prev_smooth = smoothed_velocities
                        .get(&address)
                        .map(|r| *r.value())
                        .unwrap_or(Vector3::ZERO);
                    let smoothed = Vector3::new(
                        VELOCITY_SMOOTHING * raw_velocity.x + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.x,
                        VELOCITY_SMOOTHING * raw_velocity.y + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.y,
                        VELOCITY_SMOOTHING * raw_velocity.z + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.z,
                    );
                    
                    smoothed_velocities.insert(address, smoothed);
                    
                    if let Some(mut entity) = cached_players.get_mut(&address) {
                        entity.velocity = smoothed;
                    }
                }
            }
        }
    }

    fn update_entity_positions(
        entity: &mut Entity,
        _model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
    ) -> bool {
        if entity.humanoid_address != 0 {
            let health_check = memory.read::<f32>(entity.humanoid_address + humanoid::health());
            let max_health_check = memory.read::<f32>(entity.humanoid_address + humanoid::max_health());
            
            if health_check.is_nan() || max_health_check.is_nan() 
                || max_health_check <= 0.0 || max_health_check > 100000.0 
                || !is_valid_address(entity.humanoid_address) 
            {
                return false;
            }
        }

        for part_data in entity.parts.values_mut() {
            if is_valid_address(part_data.primitive_address) {
                let pos = memory.read::<Vector3>(part_data.primitive_address + crate::core::offsets::base_part::position());
                if pos.is_valid() {
                    part_data.position = pos;
                }
                let rot = memory.read::<Matrix3>(part_data.primitive_address + crate::core::offsets::base_part::rotation());
                part_data.rotation = rot;
            }
        }
        
        if is_valid_address(entity.humanoid_address) {
            let health = memory.read::<f32>(entity.humanoid_address + humanoid::health());
            let max_health = memory.read::<f32>(entity.humanoid_address + humanoid::max_health());
            
            if !health.is_nan() && !health.is_infinite() && health >= 0.0 && health <= 100000.0 {
                entity.health = health;
            }
            if !max_health.is_nan() && !max_health.is_infinite() && max_health > 0.0 && max_health <= 100000.0 {
                entity.max_health = max_health;
            }
        }
        
        if is_valid_address(entity.body_effects_address) {
            let be = Instance::new(entity.body_effects_address, Arc::clone(memory));
            if let Some(armor_inst) = be.find_first_child("Armor") {
                let value = memory.read::<f64>(armor_inst.address + crate::core::offsets::value_base::value());
                if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                    entity.armor = value as f32;
                }
            }
        }
        // Update Rivals-style team check: Check if HumanoidRootPart has TeammateLabel child
        // This needs to be checked every update since teams can change mid-game
        if let Some(hrp_data) = entity.parts.get(&BodyPart::HumanoidRootPart) {
            let hrp_instance = Instance::new(hrp_data.address, Arc::clone(memory));
            entity.has_teammate_label = hrp_instance.find_first_child("TeammateLabel").is_some();
        }
        true
    }

    fn build_entity(
        player: &Player,
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
    ) -> Option<Entity> {
        let model_children = model.get_children();
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();

        for part_instance in model_children {
            if let Some(body_part) = BodyPart::from_name(&part_instance.get_name()) {
                let part = Part::new(part_instance.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();

                if !position.is_valid() {
                    continue;
                }

                // Cache primitive address for fast position reads (avoids find_first_child every frame)
                parts.insert(
                    body_part,
                    PartData {
                        address: part_instance.address,
                        primitive_address: primitive.address,
                        size: primitive.get_size(),
                        position,
                        rotation: primitive.get_rotation(),
                    },
                );
            }
        }

        if parts.is_empty() {
            return None;
        }

        let (rig_type, humanoid_address) = model
            .find_first_child("Humanoid")
            .map(|h| {
                let hum = Humanoid::new(h.address, Arc::clone(memory));
                (hum.get_rig_type(), h.address)
            })
            .unwrap_or((0, 0));

        let (health, max_health) = if humanoid_address != 0 {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            // Validate health values
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        let team_address = player.get_team_address();

        // Rivals-style team check: Check if HumanoidRootPart has a TeammateLabel child
        // This is more reliable than the whitelist system for games like Rivals
        let has_teammate_label = parts
            .get(&BodyPart::HumanoidRootPart)
            .map(|hrp| {
                let hrp_instance = Instance::new(hrp.address, Arc::clone(memory));
                hrp_instance.find_first_child("TeammateLabel").is_some()
            })
            .unwrap_or(false);

        let player_name = player.get_name();
        let (body_effects_address, armor, max_armor) = model
            .find_first_child("BodyEffects")
            .map(|be| {
                let be_addr = be.address;
                static BE_LOG_TIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                if now - BE_LOG_TIME.load(std::sync::atomic::Ordering::Relaxed) >= 15 {
                    tracing::info!("Found BodyEffects for player {} at 0x{:x}", player_name, be_addr);
                    BE_LOG_TIME.store(now, std::sync::atomic::Ordering::Relaxed);
                }
                
                let armor_instance = be.find_first_child("Armor");
                let (armor_val, max_armor_val) = if let Some(armor_inst) = armor_instance {
                    let value = memory.read::<f64>(armor_inst.address + crate::core::offsets::value_base::value());
                    let armor = if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                        value as f32
                    } else {
                        0.0
                    };
                    let max_armor = be.find_first_child("MaxArmor")
                        .map(|ma| {
                            let mv = memory.read::<f64>(ma.address + crate::core::offsets::value_base::value());
                            if !mv.is_nan() && !mv.is_infinite() && mv > 0.0 { mv as f32 } else { 100.0 }
                        })
                        .unwrap_or(100.0);
                    
                    tracing::debug!("Player {} armor: {:.1}/{:.1}", player_name, armor, max_armor);
                    (armor, max_armor)
                } else {
                    tracing::debug!("Player {} has BodyEffects but no Armor child", player_name);
                    (0.0, 0.0)
                };
                (be_addr, armor_val, max_armor_val)
            })
            .unwrap_or((0, 0.0, 0.0));

        Some(Entity {
            model_address: model.address(),
            name: player.get_name(),
            rig_type,
            humanoid_address,
            team_address,
            team_identifier: String::new(),
            body_effects_address,
            armor_value_address: 0,
            equipped_tool_sv_address: 0,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor,
            max_armor,
            has_teammate_label,
            is_game_specific: false,
            is_transparent: false,
            account_age: 0,
            equipped_weapon: String::new(),
        })
    }

    fn is_body_part(instance: &Instance) -> bool {
        let class = instance.get_class_name();
        let name = instance.get_name();

        class.contains("Part")
            || matches!(
                name.as_str(),
                "Head"
                    | "Torso"
                    | "HumanoidRootPart"
                    | "UpperTorso"
                    | "LowerTorso"
                    | "Left Arm"
                    | "Right Arm"
                    | "Left Leg"
                    | "Right Leg"
                    | "LeftUpperArm"
                    | "LeftLowerArm"
                    | "LeftHand"
                    | "RightUpperArm"
                    | "RightLowerArm"
                    | "RightHand"
                    | "LeftUpperLeg"
                    | "LeftLowerLeg"
                    | "LeftFoot"
                    | "RightUpperLeg"
                    | "RightLowerLeg"
                    | "RightFoot"
            )
    }

    fn cleanup_disconnected(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
        current_addresses: &AHashSet<u64>,
    ) {
        cached_players.retain(|addr, entity| {
            if !current_addresses.contains(addr) {
                return false;
            }
            let has_valid_pos = entity.parts.values().any(|p| {
                p.position.is_valid() && !p.position.is_near_origin(5.0)
            });
            has_valid_pos
        });
        position_history.retain(|addr, _| current_addresses.contains(addr));
        smoothed_velocities.retain(|addr, _| current_addresses.contains(addr));
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.stop();
    }
}
