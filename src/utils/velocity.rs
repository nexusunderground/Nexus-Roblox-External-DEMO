//! Shared velocity tracking for prediction across all aim systems.
//! 
//! Two variants:
//! - `VelocityTracker`: Simple EMA with adaptive alpha + acceleration (camera_aim, viewport, silentaim)
//! - `RingVelocityTracker`: Ring-buffer EMA with stationary detection (targeting/aim_assist)

use std::time::Instant;
use crate::utils::math::Vector3;

// ============================================================================
// Constants
// ============================================================================

/// Minimum velocity to consider "moving" (studs/sec)
pub const VELOCITY_THRESHOLD: f32 = 0.5;

/// EMA alpha for velocity smoothing (lower = smoother, higher = more responsive)
pub const VELOCITY_EMA_ALPHA: f32 = 0.32;

/// EMA alpha for acceleration smoothing
pub const ACCEL_EMA_ALPHA: f32 = 0.22;

/// Speed above this is rejected as teleport/glitch (studs/sec)
pub const VELOCITY_OUTLIER_THRESHOLD: f32 = 80.0;

/// Speed below which target is considered stationary (studs/sec)
pub const STATIONARY_THRESHOLD: f32 = 1.5;

/// Position interpolation time step (seconds) for visual smoothing.
/// Used by ESP and chams to interpolate positions between cache updates.
pub const INTERPOLATION_TIME: f32 = 0.008;

// ============================================================================
// Simple VelocityTracker (used by camera_aim, viewport, silentaim)
// ============================================================================

/// Simple EMA velocity + acceleration tracker.
/// Lightweight, suitable for per-entity tracking in any aim system.
pub struct VelocityTracker {
    last_pos: Vector3,
    last_time: Instant,
    velocity: Vector3,
    acceleration: Vector3,
    samples: u32,
}

impl VelocityTracker {
    pub fn new(pos: Vector3) -> Self {
        Self {
            last_pos: pos,
            last_time: Instant::now(),
            velocity: Vector3::ZERO,
            acceleration: Vector3::ZERO,
            samples: 0,
        }
    }

    /// Update tracker with new position. Returns (velocity, acceleration).
    pub fn update(&mut self, pos: Vector3) -> (Vector3, Vector3) {
        let dt = self.last_time.elapsed().as_secs_f32();
        if dt > 0.001 && dt < 0.1 {
            let instant_vel = (pos - self.last_pos) / dt;

            // Outlier rejection — skip teleport/respawn spikes
            if instant_vel.length() > VELOCITY_OUTLIER_THRESHOLD {
                self.last_pos = pos;
                self.last_time = Instant::now();
                return (self.velocity, self.acceleration);
            }

            if self.samples > 0 {
                // Track acceleration (change in velocity)
                let instant_accel = (instant_vel - self.velocity) / dt.max(0.005);
                self.acceleration = Vector3::new(
                    self.acceleration.x + ACCEL_EMA_ALPHA * (instant_accel.x - self.acceleration.x),
                    self.acceleration.y + ACCEL_EMA_ALPHA * (instant_accel.y - self.acceleration.y),
                    self.acceleration.z + ACCEL_EMA_ALPHA * (instant_accel.z - self.acceleration.z),
                );

                // Smooth velocity with adaptive alpha
                let vel_change = (instant_vel - self.velocity).length();
                let adaptive_alpha = VELOCITY_EMA_ALPHA * (0.8 + 0.4 / (1.0 + vel_change * 0.05));
                self.velocity = Vector3::new(
                    self.velocity.x + adaptive_alpha * (instant_vel.x - self.velocity.x),
                    self.velocity.y + adaptive_alpha * (instant_vel.y - self.velocity.y),
                    self.velocity.z + adaptive_alpha * (instant_vel.z - self.velocity.z),
                );
            } else {
                self.velocity = instant_vel;
            }
            self.last_pos = pos;
            self.last_time = Instant::now();
            self.samples = self.samples.saturating_add(1);
        }
        (self.velocity, self.acceleration)
    }

    /// Predict position using velocity + acceleration (quadratic: pos + vel*t + 0.5*accel*t²)
    pub fn predict(&self, current_pos: Vector3, time_ahead: f32) -> Vector3 {
        Vector3::new(
            current_pos.x + self.velocity.x * time_ahead + 0.5 * self.acceleration.x * time_ahead * time_ahead,
            current_pos.y + self.velocity.y * time_ahead + 0.5 * self.acceleration.y * time_ahead * time_ahead,
            current_pos.z + self.velocity.z * time_ahead + 0.5 * self.acceleration.z * time_ahead * time_ahead,
        )
    }

    /// Returns seconds since this tracker was last updated.
    /// Used to prune stale per-entity trackers when players disconnect.
    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.last_time.elapsed().as_secs_f32()
    }
}

// ============================================================================
// Ring-buffer VelocityTracker (used by targeting/aim_assist)
// ============================================================================

/// Advanced ring-buffer EMA velocity tracker with stationary detection.
/// Uses 5-sample history for better smoothing and outlier handling.
pub struct RingVelocityTracker {
    positions: [(Vector3, Instant); 5],
    index: usize,
    velocity: Vector3,
    prev_velocity: Vector3,
    acceleration: Vector3,
    samples: u32,
    is_stationary: bool,
    stationary_frames: u32,
}

/// EMA alpha for the ring-buffer tracker (slightly tighter than simple tracker)
const RING_VELOCITY_EMA_ALPHA: f32 = 0.28;

impl RingVelocityTracker {
    pub fn new(position: Vector3) -> Self {
        let now = Instant::now();
        Self {
            positions: [(position, now); 5],
            index: 0,
            velocity: Vector3::ZERO,
            prev_velocity: Vector3::ZERO,
            acceleration: Vector3::ZERO,
            samples: 0,
            is_stationary: true,
            stationary_frames: 0,
        }
    }

    /// Update tracker with new position. Returns smoothed velocity.
    pub fn update(&mut self, current_pos: Vector3) -> Vector3 {
        let now = Instant::now();
        let prev_idx = if self.index == 0 { 4 } else { self.index - 1 };
        let (prev_pos, prev_time) = self.positions[prev_idx];

        let dt = now.duration_since(prev_time).as_secs_f32();

        if dt > 0.001 && dt < 0.1 {
            let instant_vel = (current_pos - prev_pos) / dt;

            // Outlier rejection — ignore teleports/glitches
            let speed = instant_vel.length();
            if speed > VELOCITY_OUTLIER_THRESHOLD {
                self.positions[self.index] = (current_pos, now);
                self.index = (self.index + 1) % 5;
                return self.velocity;
            }

            // Apply adaptive EMA smoothing
            let alpha = if self.samples > 2 {
                let vel_change = (instant_vel - self.velocity).length();
                let consistency = 1.0 / (1.0 + vel_change * 0.1);
                RING_VELOCITY_EMA_ALPHA * (0.7 + 0.6 * consistency)
            } else {
                RING_VELOCITY_EMA_ALPHA
            };

            if self.samples > 0 {
                self.prev_velocity = self.velocity;
                self.velocity = Vector3::new(
                    self.velocity.x + alpha * (instant_vel.x - self.velocity.x),
                    self.velocity.y + alpha * (instant_vel.y - self.velocity.y),
                    self.velocity.z + alpha * (instant_vel.z - self.velocity.z),
                );
            } else {
                self.velocity = instant_vel;
                self.prev_velocity = instant_vel;
            }

            // Compute acceleration from velocity difference
            if self.samples >= 2 && dt > 0.005 {
                let new_accel = (self.velocity - self.prev_velocity) / dt;
                let max_accel = 60.0;
                self.acceleration = Vector3::new(
                    self.acceleration.x * 0.3 + new_accel.x.clamp(-max_accel, max_accel) * 0.7,
                    self.acceleration.y * 0.3 + new_accel.y.clamp(-max_accel, max_accel) * 0.7,
                    self.acceleration.z * 0.3 + new_accel.z.clamp(-max_accel, max_accel) * 0.7,
                );
            }

            // Stationary detection
            let current_speed = self.velocity.length();
            if current_speed < STATIONARY_THRESHOLD {
                self.stationary_frames = self.stationary_frames.saturating_add(1);
                self.is_stationary = self.stationary_frames > 3;
            } else {
                self.stationary_frames = 0;
                self.is_stationary = false;
            }

            // Store in ring buffer
            self.positions[self.index] = (current_pos, now);
            self.index = (self.index + 1) % 5;
            self.samples = (self.samples + 1).min(20);
        }

        self.velocity
    }

    pub fn get_acceleration(&self) -> Vector3 {
        self.acceleration
    }

}

// ============================================================================
// Shared team check utility
// ============================================================================

use crate::utils::cache::Entity;
use ahash::AHashSet;

/// Teammate check — returns true if entity is on the same team as local player.
/// Uses standard Roblox Teams (team_address match) + has_teammate_label fallback + whitelist.
#[inline]
pub fn is_teammate(
    entity: &Entity,
    team_check_enabled: bool,
    local_team: u64,
    teammate_addresses: &AHashSet<u64>,
) -> bool {
    if !team_check_enabled {
        return false;
    }

    // Whitelist (manual name → model address lookup — always applies)
    if teammate_addresses.contains(&entity.model_address) {
        return true;
    }

    // Standard Roblox Teams: same team address = teammate
    if local_team != 0 && entity.team_address == local_team {
        return true;
    }

    // TeammateLabel fallback (game-specific detection stored during cache build)
    if entity.has_teammate_label {
        return true;
    }

    false
}
