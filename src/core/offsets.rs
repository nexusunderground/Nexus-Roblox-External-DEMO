//! Dynamic Offset System - Dual Source with Fallbacks
//! 
//! Primary source: imtheo.lol (namespaced C++ format)
//! Secondary source: ntgetwritewatch.workers.dev (flat format)
//! 
//! All offsets are loaded dynamically - NO HARDCODED VALUES except for
//! calculated fallbacks that derive from other known offsets.

#![allow(dead_code)]

use std::sync::Mutex;
use ahash::AHashSet;
use crate::core::offset_loader;

/// Set of (namespace, name) pairs we've already warned about — prevents log spam.
static WARNED_OFFSETS: std::sync::LazyLock<Mutex<AHashSet<(String, String)>>> =
    std::sync::LazyLock::new(|| Mutex::new(AHashSet::new()));

pub fn client_version() -> String {
    offset_loader::get_version()
}

/// Get an offset dynamically - logs a warning ONCE per key and returns 0 if not found
pub(crate) fn get_offset(namespace: &str, name: &str) -> u64 {
    match offset_loader::get_offset(namespace, name) {
        Some(v) => v as u64,
        None => {
            if let Ok(mut set) = WARNED_OFFSETS.lock() {
                let key = (namespace.to_owned(), name.to_owned());
                if set.insert(key) {
                    tracing::warn!("Offset not found: {}::{} (returning 0)", namespace, name);
                }
            }
            0 
        }
    }
}

/// Try to get an offset - returns None if not found (for optional offsets)
pub(crate) fn try_get_offset(namespace: &str, name: &str) -> Option<u64> {
    offset_loader::get_offset(namespace, name).map(|v| v as u64)
}

/// Try multiple namespaces for an offset (for aliased offsets)
pub(crate) fn try_get_offset_multi(attempts: &[(&str, &str)]) -> Option<u64> {
    for (namespace, name) in attempts {
        if let Some(v) = offset_loader::get_offset(namespace, name) {
            return Some(v as u64);
        }
    }
    None
}

// ============================================================================
// BasePart
// ============================================================================

pub mod base_part {
    use super::{get_offset, try_get_offset, try_get_offset_multi};
    
    // --- Instance-level offsets (used as: instance_addr + offset) ---
    pub fn color3() -> u64 { get_offset("BasePart", "Color3") }
    pub fn primitive() -> u64 { get_offset("BasePart", "Primitive") }
    /// NOTE: Despite the name, this offset is applied to the **primitive** address
    /// (i.e. `prim_addr + primitive_flags()`), NOT the instance address.
    /// Falls back to CanCollide offset (same byte location) if PrimitiveFlags isn't provided.
    pub fn primitive_flags() -> u64 {
        try_get_offset_multi(&[
            ("Primitive", "Flags"),
            ("BasePart", "PrimitiveFlags"),
            ("BasePart", "CanCollide"),
            ("BasePart", "Anchored"),
        ]).unwrap_or_else(|| {
            use std::sync::atomic::{AtomicBool, Ordering};
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!("Offset not found: Primitive::Flags / BasePart::PrimitiveFlags (and no CanCollide/Anchored fallback)");
            }
            0
        })
    }
    pub fn primitive_owner() -> u64 {
        try_get_offset_multi(&[("Primitive", "Owner"), ("BasePart", "PrimitiveOwner")])
            .unwrap_or_else(|| get_offset("BasePart", "PrimitiveOwner"))
    }
    pub fn shape() -> u64 { get_offset("BasePart", "Shape") }
    pub fn transparency() -> u64 { get_offset("BasePart", "Transparency") }

    // --- Primitive-level offsets (used as: prim + offset) ---
    // The imtheo dump puts these under "Primitive" namespace, while some
    // secondary sources put them under "BasePart". Try Primitive first.
    fn prim_offset(name: &str) -> u64 {
        try_get_offset_multi(&[("Primitive", name), ("BasePart", name)]).unwrap_or_else(|| {
            // Fall through to get_offset for the warning log
            get_offset("BasePart", name)
        })
    }

    pub fn assembly_angular_velocity() -> u64 { prim_offset("AssemblyAngularVelocity") }
    pub fn assembly_linear_velocity() -> u64 { prim_offset("AssemblyLinearVelocity") }
    pub fn material() -> u64 { prim_offset("Material") }
    pub fn position() -> u64 { prim_offset("Position") }
    pub fn rotation() -> u64 { prim_offset("Rotation") }
    pub fn size() -> u64 { prim_offset("Size") }
    pub fn validate_primitive() -> u64 {
        try_get_offset_multi(&[("Primitive", "Validate"), ("Primitive", "ValidatePrimitive"), ("BasePart", "ValidatePrimitive")])
            .unwrap_or_else(|| get_offset("BasePart", "ValidatePrimitive"))
    }
    
    // Velocity - from ntgetwritewatch (same as Transparency offset in their format)
    pub fn velocity() -> u64 { 
        try_get_offset("BasePart", "Velocity").unwrap_or_else(|| assembly_linear_velocity()) 
    }
    
    // CFrame - try Primitive first, then BasePart, then fallback to Rotation offset
    pub fn cframe() -> u64 {
        try_get_offset_multi(&[("Primitive", "CFrame"), ("BasePart", "CFrame")])
            .unwrap_or_else(|| rotation())
    }
    
    // Anchored flag location (same as PrimitiveFlags but different interpretation)
    pub fn anchored() -> u64 {
        try_get_offset("BasePart", "Anchored").unwrap_or_else(|| primitive_flags())
    }
    
    // Can collide flag location
    pub fn can_collide() -> u64 {
        try_get_offset("BasePart", "CanCollide").unwrap_or_else(|| primitive_flags())
    }

    /// Collision group index byte within the primitive object (primitive-relative).
    /// Last confirmed-working value: Primitive+0x1B5 (live probe 2026-05-22).
    pub fn collision_group_index() -> u64 {
        0x1B5
    }
    
    // Material type — no hardcoded fallback; returns 0 if not in the offset dump.
    pub fn material_type() -> u64 {
        try_get_offset("BasePart", "MaterialType").unwrap_or(0)
    }
}

// ============================================================================
// ByteCode
// ============================================================================

pub mod byte_code {
    use super::try_get_offset_multi;

    pub fn pointer() -> u64 {
        try_get_offset_multi(&[
            ("ByteCode", "Pointer"),
            ("ByteCode", "ModuleScriptPointer"),
            ("ByteCode", "LocalScriptPointer"),
        ])
        .unwrap_or(0)
    }

    pub fn size() -> u64 {
        try_get_offset_multi(&[
            ("ByteCode", "Size"),
            ("ByteCode", "ModuleScriptSize"),
            ("ByteCode", "LocalScriptSize"),
        ])
        .unwrap_or(0)
    }
}

// ============================================================================
// Camera
// ============================================================================

pub mod camera {
    use super::get_offset;
    
    pub fn camera_subject() -> u64 { get_offset("Camera", "CameraSubject") }
    pub fn camera_type() -> u64 { get_offset("Camera", "CameraType") }
    pub fn field_of_view() -> u64 { get_offset("Camera", "FieldOfView") }
    pub fn position() -> u64 { get_offset("Camera", "Position") }
    pub fn rotation() -> u64 { get_offset("Camera", "Rotation") }
    pub fn viewport() -> u64 { get_offset("Camera", "Viewport") }
    pub fn viewport_size() -> u64 { get_offset("Camera", "ViewportSize") }
}

// ============================================================================
// DataModel
// ============================================================================

pub mod datamodel {
    use super::get_offset;
    
    pub fn creator_id() -> u64 { get_offset("DataModel", "CreatorId") }
    pub fn game_id() -> u64 { get_offset("DataModel", "GameId") }
    pub fn game_loaded() -> u64 { get_offset("DataModel", "GameLoaded") }
    pub fn job_id() -> u64 { get_offset("DataModel", "JobId") }
    pub fn place_id() -> u64 { get_offset("DataModel", "PlaceId") }
    pub fn place_version() -> u64 { get_offset("DataModel", "PlaceVersion") }
    pub fn primitive_count() -> u64 { get_offset("DataModel", "PrimitiveCount") }
    pub fn script_context() -> u64 { get_offset("DataModel", "ScriptContext") }
    pub fn server_ip() -> u64 { get_offset("DataModel", "ServerIP") }
    pub fn workspace() -> u64 { get_offset("DataModel", "Workspace") }
    /// Chain: DataModel + ToRenderView1 → deref + ToRenderView2 → deref + ToRenderView3 → RenderView ptr
    pub fn to_render_view1() -> u64 { get_offset("DataModel", "ToRenderView1") }
    pub fn to_render_view2() -> u64 { get_offset("DataModel", "ToRenderView2") }
    pub fn to_render_view3() -> u64 { get_offset("DataModel", "ToRenderView3") }
}

// ============================================================================
// FFlags
// ============================================================================

pub mod fflags {
    use super::try_get_offset;
    
    pub fn debug_disable_timeout_disconnect() -> u64 { try_get_offset("FFlags", "DebugDisableTimeoutDisconnect").unwrap_or(0) }
    pub fn enable_load_module() -> u64 { try_get_offset("FFlags", "EnableLoadModule").unwrap_or(0) }
    pub fn party_player_inactivity_timeout_in_seconds() -> u64 { try_get_offset("FFlags", "PartyPlayerInactivityTimeoutInSeconds").unwrap_or(0) }
    pub fn physics_sender_max_bandwidth_bps() -> u64 { try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBps").unwrap_or(0) }
    pub fn physics_sender_max_bandwidth_bps_scaling() -> u64 { try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBpsScaling").unwrap_or(0) }
    pub fn task_scheduler_target_fps() -> u64 { try_get_offset("FFlags", "TaskSchedulerTargetFps").unwrap_or(0) }
    pub fn web_socket_service_enable_client_creation() -> u64 { try_get_offset("FFlags", "WebSocketServiceEnableClientCreation").unwrap_or(0) }
    pub fn world_step_max() -> u64 { try_get_offset("FFlags", "WorldStepMax").unwrap_or(0) }
    pub fn world_steps_offset_adjust_rate() -> u64 { try_get_offset("FFlags", "WorldStepsOffsetAdjustRate").unwrap_or(0) }

    /// Returns true if FFlag offsets were successfully loaded
    pub fn is_available() -> bool {
        try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBps").is_some()
    }
}

// ============================================================================
// FakeDataModel
// ============================================================================

pub mod fake_datamodel {
    use super::get_offset;
    
    pub fn pointer() -> u64 { get_offset("FakeDataModel", "Pointer") }
    pub fn real_datamodel() -> u64 { get_offset("FakeDataModel", "RealDataModel") }
}

// ============================================================================
// GuiObject
// ============================================================================

pub mod gui_object {
    use super::{get_offset, try_get_offset_multi};
    
    pub fn background_color3() -> u64 { get_offset("GuiObject", "BackgroundColor3") }
    pub fn border_color3() -> u64 { get_offset("GuiObject", "BorderColor3") }
    pub fn image() -> u64 { get_offset("GuiObject", "Image") }
    pub fn layout_order() -> u64 { get_offset("GuiObject", "LayoutOrder") }
    pub fn position() -> u64 { get_offset("GuiObject", "Position") }
    pub fn rich_text() -> u64 { get_offset("GuiObject", "RichText") }
    pub fn rotation() -> u64 { get_offset("GuiObject", "Rotation") }
    pub fn screen_gui_enabled() -> u64 { get_offset("GuiObject", "ScreenGui_Enabled") }
    pub fn size() -> u64 { get_offset("GuiObject", "Size") }
    
    pub fn text() -> u64 {
        try_get_offset_multi(&[("GuiObject", "Text"), ("GuiObject", "TextLabelText")])
            .unwrap_or(0)
    }
    
    pub fn text_color3() -> u64 { get_offset("GuiObject", "TextColor3") }
    pub fn visible() -> u64 { get_offset("GuiObject", "Visible") }
    pub fn z_index() -> u64 { get_offset("GuiObject", "ZIndex") }
    pub fn background_transparency() -> u64 { get_offset("GuiObject", "BackgroundTransparency") }
}

// ============================================================================
// Humanoid
// ============================================================================

pub mod humanoid {
    use super::{get_offset, try_get_offset};
    
    pub fn auto_rotate() -> u64 { get_offset("Humanoid", "AutoRotate") }
    pub fn floor_material() -> u64 { get_offset("Humanoid", "FloorMaterial") }
    pub fn health() -> u64 { get_offset("Humanoid", "Health") }
    pub fn hip_height() -> u64 { get_offset("Humanoid", "HipHeight") }
    pub fn humanoid_state() -> u64 { get_offset("Humanoid", "HumanoidState") }
    pub fn humanoid_state_id() -> u64 { get_offset("Humanoid", "HumanoidStateID") }
    pub fn jump() -> u64 { get_offset("Humanoid", "Jump") }
    pub fn jump_height() -> u64 { get_offset("Humanoid", "JumpHeight") }
    pub fn jump_power() -> u64 { get_offset("Humanoid", "JumpPower") }
    pub fn max_health() -> u64 { get_offset("Humanoid", "MaxHealth") }
    pub fn max_slope_angle() -> u64 { get_offset("Humanoid", "MaxSlopeAngle") }
    pub fn move_direction() -> u64 { get_offset("Humanoid", "MoveDirection") }
    pub fn rig_type() -> u64 { get_offset("Humanoid", "RigType") }
    pub fn walkspeed() -> u64 { get_offset("Humanoid", "Walkspeed") }
    pub fn walkspeed_check() -> u64 { get_offset("Humanoid", "WalkspeedCheck") }
    
    // --- State/behavior flags ---
    pub fn sit() -> u64 { get_offset("Humanoid", "Sit") }
    
    pub fn seat_part() -> u64 {
        try_get_offset("Humanoid", "SeatPart")
            .or_else(|| try_get_offset("Humanoid", "IsSeat"))
            .unwrap_or(0)
    }
    
    /// Kept for backwards compat — prefer `seat_part()`.
    pub fn is_seat() -> u64 { seat_part() }
    
    pub fn platform_stand() -> u64 { get_offset("Humanoid", "PlatformStand") }
    
    pub fn ragdoll() -> u64 { get_offset("Humanoid", "Ragdoll") }
    
    pub fn evaluate_state_machine() -> u64 { get_offset("Humanoid", "EvaluateStateMachine") }
    
    pub fn auto_jump_enabled() -> u64 { get_offset("Humanoid", "AutoJumpEnabled") }
    
    pub fn break_joints_on_death() -> u64 { get_offset("Humanoid", "BreakJointsOnDeath") }
    
    pub fn requires_neck() -> u64 { get_offset("Humanoid", "RequiresNeck") }
    
    // --- Camera / display ---
    pub fn camera_offset() -> u64 { get_offset("Humanoid", "CameraOffset") }
    pub fn display_distance_type() -> u64 { get_offset("Humanoid", "DisplayDistanceType") }
    pub fn display_name() -> u64 { get_offset("Humanoid", "DisplayName") }
    pub fn health_display_type() -> u64 { get_offset("Humanoid", "HealthDisplayType") }
    pub fn name_occlusion() -> u64 { get_offset("Humanoid", "NameOcclusion") }
    
    pub fn health_display_distance() -> u64 { get_offset("Humanoid", "HealthDisplayDistance") }
    
    pub fn name_display_distance() -> u64 { get_offset("Humanoid", "NameDisplayDistance") }
    
    // --- Root part ---
    pub fn humanoid_root_part() -> u64 {
        try_get_offset("Humanoid", "HumanoidRootPart")
            .or_else(|| try_get_offset("Humanoid", "RootPartR6"))
            .unwrap_or(0)
    }
    
    pub fn root_part_r6() -> u64 {
        try_get_offset("Humanoid", "RootPartR6")
            .or_else(|| try_get_offset("Humanoid", "HumanoidRootPart"))
            .unwrap_or(0)
    }
    
    pub fn root_part_r15() -> u64 { get_offset("Humanoid", "RootPartR15") }
    
    // --- MoveTo offsets (used for programmatic walking) ---
    pub fn move_to_point() -> u64 { get_offset("Humanoid", "MoveToPoint") }
    pub fn walk_timer() -> u64 { get_offset("Humanoid", "WalkTimer") }
    pub fn is_walking() -> u64 { get_offset("Humanoid", "IsWalking") }
    pub fn move_to_part() -> u64 { get_offset("Humanoid", "MoveToPart") }
    pub fn target_point() -> u64 { get_offset("Humanoid", "TargetPoint") }
}

// ============================================================================
// Instance
// ============================================================================

pub mod instance {
    use super::get_offset;
    
    pub fn attribute_container() -> u64 { get_offset("Instance", "AttributeContainer") }
    pub fn attribute_list() -> u64 { get_offset("Instance", "AttributeList") }
    pub fn attribute_to_next() -> u64 { get_offset("Instance", "AttributeToNext") }
    pub fn attribute_to_value() -> u64 { get_offset("Instance", "AttributeToValue") }
    pub fn children_end() -> u64 { get_offset("Instance", "ChildrenEnd") }
    pub fn children_start() -> u64 { get_offset("Instance", "ChildrenStart") }
    pub fn class_base() -> u64 { get_offset("Instance", "ClassBase") }
    pub fn class_descriptor() -> u64 { get_offset("Instance", "ClassDescriptor") }
    pub fn class_name() -> u64 { get_offset("Instance", "ClassName") }
    pub fn name() -> u64 { get_offset("Instance", "Name") }
    pub fn parent() -> u64 { get_offset("Instance", "Parent") }
    pub fn capabilities() -> u64 { get_offset("Instance", "Capabilities") }
}

// ============================================================================
// Lighting
// ============================================================================

pub mod lighting {
    use super::get_offset;
    
    pub fn ambient() -> u64 { get_offset("Lighting", "Ambient") }
    pub fn brightness() -> u64 { get_offset("Lighting", "Brightness") }
    pub fn clock_time() -> u64 { get_offset("Lighting", "ClockTime") }
    pub fn color_shift_bottom() -> u64 { get_offset("Lighting", "ColorShift_Bottom") }
    pub fn color_shift_top() -> u64 { get_offset("Lighting", "ColorShift_Top") }
    pub fn exposure_compensation() -> u64 { get_offset("Lighting", "ExposureCompensation") }
    pub fn fog_color() -> u64 { get_offset("Lighting", "FogColor") }
    pub fn fog_end() -> u64 { get_offset("Lighting", "FogEnd") }
    pub fn fog_start() -> u64 { get_offset("Lighting", "FogStart") }
    pub fn geographic_latitude() -> u64 { get_offset("Lighting", "GeographicLatitude") }
    pub fn outdoor_ambient() -> u64 { get_offset("Lighting", "OutdoorAmbient") }
    pub fn global_shadows() -> u64 { get_offset("Lighting", "GlobalShadows") }
    pub fn shadow_color() -> u64 { get_offset("Lighting", "ShadowColor") }
    pub fn environment_diffuse_scale() -> u64 { get_offset("Lighting", "EnvironmentDiffuseScale") }
    pub fn environment_specular_scale() -> u64 { get_offset("Lighting", "EnvironmentSpecularScale") }
    pub fn gradient_bottom() -> u64 { get_offset("Lighting", "GradientBottom") }
    pub fn gradient_top() -> u64 { get_offset("Lighting", "GradientTop") }
    pub fn light_color() -> u64 { get_offset("Lighting", "LightColor") }
    pub fn light_direction() -> u64 { get_offset("Lighting", "LightDirection") }
    pub fn moon_position() -> u64 { get_offset("Lighting", "MoonPosition") }
    pub fn sun_position() -> u64 { get_offset("Lighting", "SunPosition") }
    pub fn sky() -> u64 { get_offset("Lighting", "Sky") }
    pub fn source() -> u64 { get_offset("Lighting", "Source") }
}

// ============================================================================
// Terrain
// ============================================================================

pub mod terrain {
    use super::get_offset;
    
    pub fn grass_length() -> u64 { get_offset("Terrain", "GrassLength") }
    pub fn water_color() -> u64 { get_offset("Terrain", "WaterColor") }
    pub fn water_reflectance() -> u64 { get_offset("Terrain", "WaterReflectance") }
    pub fn water_transparency() -> u64 { get_offset("Terrain", "WaterTransparency") }
    pub fn water_wave_size() -> u64 { get_offset("Terrain", "WaterWaveSize") }
    pub fn water_wave_speed() -> u64 { get_offset("Terrain", "WaterWaveSpeed") }
}

// ============================================================================
// LocalScript
// ============================================================================

pub mod local_script {
    use super::get_offset;

    pub fn byte_code() -> u64 { get_offset("LocalScript", "ByteCode") }
    pub fn guid() -> u64 { get_offset("LocalScript", "GUID") }
    pub fn hash() -> u64 { get_offset("LocalScript", "Hash") }
}

// ============================================================================
// MeshPart
// ============================================================================

pub mod mesh_part {
    use super::get_offset;
    
    pub fn mesh_id() -> u64 { get_offset("MeshPart", "MeshId") }
    pub fn texture() -> u64 { get_offset("MeshPart", "Texture") }
    pub fn binding() -> u64 { get_offset("MeshPart", "Binding") }
}

// ============================================================================
// Misc
// ============================================================================

pub mod misc {
    use super::get_offset;
    
    pub fn adornee() -> u64 { get_offset("Misc", "Adornee") }
    pub fn animation_id() -> u64 { get_offset("Misc", "AnimationId") }
    pub fn require_lock() -> u64 { get_offset("Misc", "RequireLock") }
    pub fn string_length() -> u64 { get_offset("Misc", "StringLength") }
    pub fn value() -> u64 { get_offset("Misc", "Value") }
}

// ============================================================================
// Model
// ============================================================================

pub mod model {
    use super::get_offset;
    
    pub fn primary_part() -> u64 { get_offset("Model", "PrimaryPart") }
    pub fn scale() -> u64 { get_offset("Model", "Scale") }
}

// ============================================================================
// ModuleScript
// ============================================================================

pub mod module_script {
    use super::{get_offset, try_get_offset};

    pub fn byte_code() -> u64 { get_offset("ModuleScript", "ByteCode") }

    pub fn is_core_script() -> u64 {
        try_get_offset("ModuleScript", "IsCoreScript")
            .or_else(|| try_get_offset("ModuleScript", "ModuleType"))
            .unwrap_or(0)
    }
}

// ============================================================================
// MouseService
// ============================================================================

pub mod mouse_service {
    use super::get_offset;
    
    pub fn input_object() -> u64 { get_offset("MouseService", "InputObject") }
    pub fn input_object2() -> u64 { get_offset("MouseService", "InputObject2") }
    pub fn mouse_position() -> u64 { get_offset("MouseService", "MousePosition") }
    pub fn sensitivity_pointer() -> u64 { get_offset("MouseService", "SensitivityPointer") }
}

// ============================================================================
// Player
// ============================================================================

pub mod player {
    use super::get_offset;
    
    pub fn camera_mode() -> u64 { get_offset("Player", "CameraMode") }
    pub fn country() -> u64 { get_offset("Player", "Country") }
    pub fn display_name() -> u64 { get_offset("Player", "DisplayName") }
    pub fn gender() -> u64 { get_offset("Player", "Gender") }
    pub fn localplayer() -> u64 { get_offset("Player", "LocalPlayer") }
    pub fn max_zoom_distance() -> u64 { get_offset("Player", "MaxZoomDistance") }
    pub fn min_zoom_distance() -> u64 { get_offset("Player", "MinZoomDistance") }
    pub fn model_instance() -> u64 { get_offset("Player", "ModelInstance") }
    pub fn mouse() -> u64 { get_offset("Player", "Mouse") }
    
    pub fn team() -> u64 { get_offset("Player", "Team") }
    
    pub fn user_id() -> u64 { get_offset("Player", "UserId") }

    pub fn account_age() -> u64 { get_offset("Player", "AccountAge") }

    pub fn character_appearance_id() -> u64 { get_offset("Player", "CharacterAppearanceId") }
    
    pub fn team_color() -> u64 { get_offset("Player", "TeamColor") }
}

// ============================================================================
// PrimitiveFlags
// ============================================================================

pub mod primitive_flags {
    use super::{get_offset, try_get_offset_multi};
    
    pub fn anchored() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "Anchored"),
            ("PrimitiveFlags", "AnchoredMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "Anchored"))
    }
    pub fn can_collide() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "CanCollide"),
            ("PrimitiveFlags", "CanCollideMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "CanCollide"))
    }
    pub fn can_touch() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "CanTouch"),
            ("PrimitiveFlags", "CanTouchMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "CanTouch"))
    }
    pub fn can_query() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "CanQuery"),
            ("PrimitiveFlags", "CanQueryMask"),
        ]).unwrap_or(0)
    }
}

// ============================================================================
// ProximityPrompt
// ============================================================================

pub mod proximity_prompt {
    use super::get_offset;
    
    pub fn action_text() -> u64 { get_offset("ProximityPrompt", "ActionText") }
    pub fn enabled() -> u64 { get_offset("ProximityPrompt", "Enabled") }
    pub fn gamepad_key_code() -> u64 { get_offset("ProximityPrompt", "GamepadKeyCode") }
    pub fn hold_duration() -> u64 { get_offset("ProximityPrompt", "HoldDuration") }
    pub fn key_code() -> u64 { get_offset("ProximityPrompt", "KeyCode") }
    pub fn max_activation_distance() -> u64 { get_offset("ProximityPrompt", "MaxActivationDistance") }
    pub fn object_text() -> u64 { get_offset("ProximityPrompt", "ObjectText") }
    pub fn requires_line_of_sight() -> u64 { get_offset("ProximityPrompt", "RequiresLineOfSight") }
}

// ============================================================================
// RenderView
// ============================================================================

pub mod render_view {
    use super::get_offset;
    
    pub fn device_d3d11() -> u64 { get_offset("RenderView", "DeviceD3D11") }
    pub fn visual_engine() -> u64 { get_offset("RenderView", "VisualEngine") }
    pub fn lighting_valid() -> u64 { get_offset("RenderView", "LightingValid") }
    pub fn sky_valid() -> u64 { get_offset("RenderView", "SkyValid") }
}

// ============================================================================
// RunService
// ============================================================================

pub mod run_service {
    use super::get_offset;
    
    pub fn heartbeat_fps() -> u64 { get_offset("RunService", "HeartbeatFPS") }
    pub fn heartbeat_task() -> u64 { get_offset("RunService", "HeartbeatTask") }
}

// ============================================================================
// Sky
// ============================================================================

pub mod sky {
    use super::get_offset;
    
    pub fn moon_angular_size() -> u64 { get_offset("Sky", "MoonAngularSize") }
    pub fn moon_texture_id() -> u64 { get_offset("Sky", "MoonTextureId") }
    pub fn skybox_bk() -> u64 { get_offset("Sky", "SkyboxBk") }
    pub fn skybox_dn() -> u64 { get_offset("Sky", "SkyboxDn") }
    pub fn skybox_ft() -> u64 { get_offset("Sky", "SkyboxFt") }
    pub fn skybox_lf() -> u64 { get_offset("Sky", "SkyboxLf") }
    pub fn skybox_orientation() -> u64 { get_offset("Sky", "SkyboxOrientation") }
    pub fn skybox_rt() -> u64 { get_offset("Sky", "SkyboxRt") }
    pub fn skybox_up() -> u64 { get_offset("Sky", "SkyboxUp") }
    pub fn star_count() -> u64 { get_offset("Sky", "StarCount") }
    pub fn sun_angular_size() -> u64 { get_offset("Sky", "SunAngularSize") }
    pub fn sun_texture_id() -> u64 { get_offset("Sky", "SunTextureId") }
}

// ============================================================================
// SpecialMesh
// ============================================================================

pub mod special_mesh {
    use super::get_offset;
    
    pub fn mesh_id() -> u64 { get_offset("SpecialMesh", "MeshId") }
    pub fn scale() -> u64 { get_offset("SpecialMesh", "Scale") }
}

// ============================================================================
// TaskScheduler
// ============================================================================

pub mod task_scheduler {
    use super::get_offset;
    
    pub fn fake_datamodel_to_datamodel() -> u64 { get_offset("TaskScheduler", "FakeDataModelToDataModel") }
    pub fn job_end() -> u64 { get_offset("TaskScheduler", "JobEnd") }
    pub fn job_name() -> u64 { get_offset("TaskScheduler", "JobName") }
    pub fn job_start() -> u64 { get_offset("TaskScheduler", "JobStart") }
    pub fn max_fps() -> u64 { get_offset("TaskScheduler", "MaxFPS") }
    pub fn pointer() -> u64 { get_offset("TaskScheduler", "Pointer") }
    pub fn render_job_to_fake_datamodel() -> u64 { get_offset("TaskScheduler", "RenderJobToFakeDataModel") }
    pub fn render_job_to_render_view() -> u64 { get_offset("TaskScheduler", "RenderJobToRenderView") }
}

// ============================================================================
// Team
// ============================================================================

pub mod team {
    use super::get_offset;
    
    pub fn brick_color() -> u64 { get_offset("Team", "BrickColor") }
}

// ============================================================================
// VehicleSeat
// ============================================================================

pub mod vehicle_seat {
    use super::get_offset;
    
    pub fn max_speed() -> u64 { get_offset("VehicleSeat", "MaxSpeed") }
    pub fn steer_float() -> u64 { get_offset("VehicleSeat", "SteerFloat") }
    pub fn throttle_float() -> u64 { get_offset("VehicleSeat", "ThrottleFloat") }
    pub fn torque() -> u64 { get_offset("VehicleSeat", "Torque") }
    pub fn turn_speed() -> u64 { get_offset("VehicleSeat", "TurnSpeed") }
}

// ============================================================================
// VisualEngine
// ============================================================================

pub mod visual_engine {
    use super::get_offset;
    
    pub fn dimensions() -> u64 { get_offset("VisualEngine", "Dimensions") }
    pub fn fake_datamodel() -> u64 { get_offset("VisualEngine", "FakeDataModel") }
    pub fn pointer() -> u64 { get_offset("VisualEngine", "Pointer") }
    pub fn render_view() -> u64 { get_offset("VisualEngine", "RenderView") }
    pub fn to_datamodel1() -> u64 { get_offset("VisualEngine", "ToDataModel1") }
    pub fn to_datamodel2() -> u64 { get_offset("VisualEngine", "ToDataModel2") }
    pub fn view_matrix() -> u64 { get_offset("VisualEngine", "ViewMatrix") }
}

// ============================================================================
// Workspace
// ============================================================================

pub mod workspace {
    use super::get_offset;
    
    pub fn current_camera() -> u64 { get_offset("Workspace", "CurrentCamera") }
    pub fn distributed_game_time() -> u64 { get_offset("Workspace", "DistributedGameTime") }
    pub fn gravity_container() -> u64 { get_offset("Workspace", "GravityContainer") }
    pub fn primitives_pointer1() -> u64 { get_offset("Workspace", "PrimitivesPointer1") }
    pub fn primitives_pointer2() -> u64 { get_offset("Workspace", "PrimitivesPointer2") }
    // Published by imtheo as "ReadOnlyGravity" — this is the correct gravity field to read/write.
    pub fn read_only_gravity() -> u64 { get_offset("Workspace", "ReadOnlyGravity") }
    // Published by imtheo as "World" — pointer to the physics World object.
    pub fn world() -> u64 { get_offset("Workspace", "World") }
}

// ============================================================================
// Primitive (physics object - uses BasePart offsets for velocity/position)
// ============================================================================

pub mod primitive {
    use super::base_part;
    
    /// Velocity offset - same as BasePart::AssemblyLinearVelocity
    pub fn velocity() -> u64 { base_part::assembly_linear_velocity() }
    
    /// Position offset - same as BasePart::Position
    pub fn position() -> u64 { base_part::position() }
}

// ============================================================================
// Legacy compatibility aliases (for existing code)
// ============================================================================

pub mod value_base {
    use super::misc;
    
    /// Value offset - uses Misc::Value
    pub fn value() -> u64 { misc::value() }
}

pub mod frame {
    use super::gui_object;
    
    /// Position X - uses GuiObject::Position offset
    pub fn position_x() -> u64 { gui_object::position() }
    
    /// Position Y - offset from position_x by 0x8 (standard UDim2 layout)
    pub fn position_y() -> u64 { gui_object::position() + 0x8 }
}

// ============================================================================
// World (physics world - accessed via Workspace::World)
// ============================================================================

pub mod world {
    use super::get_offset;
    
    pub fn air_properties() -> u64 { get_offset("World", "AirProperties") }
    pub fn fallen_parts_destroy_height() -> u64 { get_offset("World", "FallenPartsDestroyHeight") }
    // Published by imtheo under namespace "World" → extra["world"]["Gravity"].
    pub fn gravity() -> u64 { get_offset("World", "Gravity") }
    /// World::primitives std::vector (data_ptr, end_ptr, cap_ptr) — gives us live Primitive* list.
    /// imtheo.lol does not publish this (physics internal). Fallback = 0x280 from live probe 2026-05-12.
    pub fn primitives() -> u64 {
        super::try_get_offset("World", "Primitives").unwrap_or(0x280)
    }
    pub fn world_steps_per_sec() -> u64 { get_offset("World", "worldStepsPerSec") }
}

// ============================================================================
// Phase 1 — newly wrapped namespaces from imtheo.lol dump
// ============================================================================

// Atmosphere (child of Lighting)
pub mod atmosphere {
    use super::get_offset;
    pub fn color() -> u64 { get_offset("Atmosphere", "Color") }
    pub fn decay() -> u64 { get_offset("Atmosphere", "Decay") }
    pub fn density() -> u64 { get_offset("Atmosphere", "Density") }
    pub fn glare() -> u64 { get_offset("Atmosphere", "Glare") }
    pub fn haze() -> u64 { get_offset("Atmosphere", "Haze") }
    pub fn offset() -> u64 { get_offset("Atmosphere", "Offset") }
}

/// Enabled offset shared by all post-processing effect instances.
/// All post-FX classes (BlurEffect, ColorCorrectionEffect, etc.) share the same layout.
/// BlurEffect is the canonical source in the dump.
pub fn post_fx_enabled_offset() -> u64 { blur_effect::enabled() }

pub mod blur_effect {
    use super::get_offset;
    pub fn enabled() -> u64 { get_offset("BlurEffect", "Enabled") }
    pub fn size() -> u64 { get_offset("BlurEffect", "Size") }
}

pub mod color_correction_effect {
    use super::get_offset;
    pub fn brightness() -> u64 { get_offset("ColorCorrectionEffect", "Brightness") }
    pub fn contrast() -> u64 { get_offset("ColorCorrectionEffect", "Contrast") }
    pub fn enabled() -> u64 { get_offset("ColorCorrectionEffect", "Enabled") }
    pub fn tint_color() -> u64 { get_offset("ColorCorrectionEffect", "TintColor") }
}

pub mod color_grading_effect {
    use super::get_offset;
    pub fn enabled() -> u64 { get_offset("ColorGradingEffect", "Enabled") }
    pub fn tonemapper_preset() -> u64 { get_offset("ColorGradingEffect", "TonemapperPreset") }
}

// AnimationTrack / Animator
pub mod animation_track {
    use super::get_offset;
    pub fn animation() -> u64 { get_offset("AnimationTrack", "Animation") }
    pub fn animator() -> u64 { get_offset("AnimationTrack", "Animator") }
    pub fn is_playing() -> u64 { get_offset("AnimationTrack", "IsPlaying") }
    pub fn looped() -> u64 { get_offset("AnimationTrack", "Looped") }
    pub fn speed() -> u64 { get_offset("AnimationTrack", "Speed") }
    pub fn time_position() -> u64 { get_offset("AnimationTrack", "TimePosition") }
}

pub mod animator {
    use super::get_offset;
    pub fn active_animations() -> u64 { get_offset("Animator", "ActiveAnimations") }
}

// ParticleEmitter
pub mod particle_emitter {
    use super::get_offset;
    pub fn acceleration() -> u64 { get_offset("ParticleEmitter", "Acceleration") }
    pub fn brightness() -> u64 { get_offset("ParticleEmitter", "Brightness") }
    pub fn drag() -> u64 { get_offset("ParticleEmitter", "Drag") }
    pub fn lifetime() -> u64 { get_offset("ParticleEmitter", "Lifetime") }
    pub fn light_emission() -> u64 { get_offset("ParticleEmitter", "LightEmission") }
    pub fn light_influence() -> u64 { get_offset("ParticleEmitter", "LightInfluence") }
    pub fn rate() -> u64 { get_offset("ParticleEmitter", "Rate") }
    pub fn rot_speed() -> u64 { get_offset("ParticleEmitter", "RotSpeed") }
    pub fn rotation() -> u64 { get_offset("ParticleEmitter", "Rotation") }
    pub fn speed() -> u64 { get_offset("ParticleEmitter", "Speed") }
    pub fn spread_angle() -> u64 { get_offset("ParticleEmitter", "SpreadAngle") }
    pub fn texture() -> u64 { get_offset("ParticleEmitter", "Texture") }
    pub fn time_scale() -> u64 { get_offset("ParticleEmitter", "TimeScale") }
    pub fn velocity_inheritance() -> u64 { get_offset("ParticleEmitter", "VelocityInheritance") }
    pub fn z_offset() -> u64 { get_offset("ParticleEmitter", "ZOffset") }
}

// Sound
pub mod sound {
    use super::get_offset;
    pub fn looped() -> u64 { get_offset("Sound", "Looped") }
    pub fn playback_speed() -> u64 { get_offset("Sound", "PlaybackSpeed") }
    pub fn playing() -> u64 { get_offset("Sound", "Playing") }
    pub fn roll_off_max_distance() -> u64 { get_offset("Sound", "RollOffMaxDistance") }
    pub fn roll_off_min_distance() -> u64 { get_offset("Sound", "RollOffMinDistance") }
    pub fn sound_group() -> u64 { get_offset("Sound", "SoundGroup") }
    pub fn sound_id() -> u64 { get_offset("Sound", "SoundId") }
    pub fn volume() -> u64 { get_offset("Sound", "Volume") }
}

// Tool (held tool in character)
pub mod tool {
    use super::get_offset;
    pub fn can_be_dropped() -> u64 { get_offset("Tool", "CanBeDropped") }
    pub fn enabled() -> u64 { get_offset("Tool", "Enabled") }
    pub fn grip() -> u64 { get_offset("Tool", "Grip") }
    pub fn manual_activation_only() -> u64 { get_offset("Tool", "ManualActivationOnly") }
    pub fn requires_handle() -> u64 { get_offset("Tool", "RequiresHandle") }
    pub fn texture_id() -> u64 { get_offset("Tool", "TextureId") }
    pub fn tooltip() -> u64 { get_offset("Tool", "Tooltip") }
}

// Weld / WeldConstraint
pub mod weld {
    use super::get_offset;
    pub fn part0() -> u64 { get_offset("Weld", "Part0") }
    pub fn part1() -> u64 { get_offset("Weld", "Part1") }
}

pub mod weld_constraint {
    use super::get_offset;
    pub fn part0() -> u64 { get_offset("WeldConstraint", "Part0") }
    pub fn part1() -> u64 { get_offset("WeldConstraint", "Part1") }
}

// SurfaceAppearance (PBR material on parts)
pub mod surface_appearance {
    use super::get_offset;
    pub fn alpha_mode() -> u64 { get_offset("SurfaceAppearance", "AlphaMode") }
    pub fn color() -> u64 { get_offset("SurfaceAppearance", "Color") }
    pub fn color_map() -> u64 { get_offset("SurfaceAppearance", "ColorMap") }
    pub fn emissive_mask_content() -> u64 { get_offset("SurfaceAppearance", "EmissiveMaskContent") }
    pub fn emissive_strength() -> u64 { get_offset("SurfaceAppearance", "EmissiveStrength") }
    pub fn emissive_tint() -> u64 { get_offset("SurfaceAppearance", "EmissiveTint") }
    pub fn metalness_map() -> u64 { get_offset("SurfaceAppearance", "MetalnessMap") }
    pub fn normal_map() -> u64 { get_offset("SurfaceAppearance", "NormalMap") }
    pub fn roughness_map() -> u64 { get_offset("SurfaceAppearance", "RoughnessMap") }
}

// ClickDetector
pub mod click_detector {
    use super::get_offset;
    pub fn max_activation_distance() -> u64 { get_offset("ClickDetector", "MaxActivationDistance") }
    pub fn mouse_icon() -> u64 { get_offset("ClickDetector", "MouseIcon") }
}

// DragDetector
pub mod drag_detector {
    use super::{get_offset, try_get_offset};
    /// MaxActivationDistance — how far the player can be to start dragging.
    pub fn max_activation_distance() -> u64 {
        try_get_offset("DragDetector", "MaxActivationDistance").unwrap_or(0)
    }
    pub fn max_drag_angle() -> u64 { get_offset("DragDetector", "MaxDragAngle") }
    pub fn max_drag_translation() -> u64 { get_offset("DragDetector", "MaxDragTranslation") }
    pub fn max_force() -> u64 { get_offset("DragDetector", "MaxForce") }
    pub fn max_torque() -> u64 { get_offset("DragDetector", "MaxTorque") }
    pub fn min_drag_angle() -> u64 { get_offset("DragDetector", "MinDragAngle") }
    pub fn min_drag_translation() -> u64 { get_offset("DragDetector", "MinDragTranslation") }
    pub fn responsiveness() -> u64 { get_offset("DragDetector", "Responsiveness") }
    pub fn reference_instance() -> u64 { get_offset("DragDetector", "ReferenceInstance") }
}

// PlayerConfigurer (global pointer for direct LocalPlayer resolution)
pub mod player_configurer {
    use super::get_offset;
    /// Static module-relative address: `base_addr + pointer()` → PlayerConfigurer object.
    pub fn pointer() -> u64 { get_offset("PlayerConfigurer", "Pointer") }
}

// PlayerMouse
pub mod player_mouse {
    use super::get_offset;
    pub fn icon() -> u64 { get_offset("PlayerMouse", "Icon") }
    pub fn workspace() -> u64 { get_offset("PlayerMouse", "Workspace") }
}

// StatsItem (Stats service children — ping/memory/fps)
pub mod stats_item {
    use super::get_offset;
    pub fn value() -> u64 { get_offset("StatsItem", "Value") }
}

// UserInputService / WindowInputState
pub mod user_input_service {
    use super::get_offset;
    /// Offset to the WindowInputState pointer within UserInputService.
    pub fn window_input_state() -> u64 { get_offset("UserInputService", "WindowInputState") }
}

pub mod window_input_state {
    use super::get_offset;
    pub fn caps_lock() -> u64 { get_offset("WindowInputState", "CapsLock") }
    /// Non-null pointer = user is currently focused in a TextBox (chat, search, etc.)
    pub fn current_text_box() -> u64 { get_offset("WindowInputState", "CurrentTextBox") }
}

// RenderJob (separate from TaskScheduler — render thread's DM view)
pub mod render_job {
    use super::get_offset;
    pub fn fake_datamodel() -> u64 { get_offset("RenderJob", "FakeDataModel") }
    pub fn real_datamodel() -> u64 { get_offset("RenderJob", "RealDataModel") }
    pub fn render_view() -> u64 { get_offset("RenderJob", "RenderView") }
}

// MaterialColors (per-material terrain color slots — byte offsets into MaterialColors object)
pub mod material_colors {
    use super::get_offset;
    pub fn asphalt() -> u64 { get_offset("MaterialColors", "Asphalt") }
    pub fn basalt() -> u64 { get_offset("MaterialColors", "Basalt") }
    pub fn brick() -> u64 { get_offset("MaterialColors", "Brick") }
    pub fn cobblestone() -> u64 { get_offset("MaterialColors", "Cobblestone") }
    pub fn concrete() -> u64 { get_offset("MaterialColors", "Concrete") }
    pub fn cracked_lava() -> u64 { get_offset("MaterialColors", "CrackedLava") }
    pub fn glacier() -> u64 { get_offset("MaterialColors", "Glacier") }
    pub fn grass() -> u64 { get_offset("MaterialColors", "Grass") }
    pub fn ground() -> u64 { get_offset("MaterialColors", "Ground") }
    pub fn ice() -> u64 { get_offset("MaterialColors", "Ice") }
    pub fn leafy_grass() -> u64 { get_offset("MaterialColors", "LeafyGrass") }
    pub fn limestone() -> u64 { get_offset("MaterialColors", "Limestone") }
    pub fn mud() -> u64 { get_offset("MaterialColors", "Mud") }
    pub fn pavement() -> u64 { get_offset("MaterialColors", "Pavement") }
    pub fn rock() -> u64 { get_offset("MaterialColors", "Rock") }
    pub fn salt() -> u64 { get_offset("MaterialColors", "Salt") }
    pub fn sand() -> u64 { get_offset("MaterialColors", "Sand") }
    pub fn sandstone() -> u64 { get_offset("MaterialColors", "Sandstone") }
    pub fn slate() -> u64 { get_offset("MaterialColors", "Slate") }
    pub fn snow() -> u64 { get_offset("MaterialColors", "Snow") }
    pub fn wood_planks() -> u64 { get_offset("MaterialColors", "WoodPlanks") }
}

// Attachment (position within parent part)
pub mod attachment {
    use super::get_offset;
    pub fn position() -> u64 { get_offset("Attachment", "Position") }
}

// AirProperties (child of World — wind/density)
pub mod air_properties {
    use super::get_offset;
    pub fn air_density() -> u64 { get_offset("AirProperties", "AirDensity") }
    pub fn global_wind() -> u64 { get_offset("AirProperties", "GlobalWind") }
}

// Seat
pub mod seat {
    use super::get_offset;
    pub fn occupant() -> u64 { get_offset("Seat", "Occupant") }
}

// SpawnLocation
pub mod spawn_location {
    use super::get_offset;
    pub fn allow_team_change_on_touch() -> u64 { get_offset("SpawnLocation", "AllowTeamChangeOnTouch") }
    pub fn enabled() -> u64 { get_offset("SpawnLocation", "Enabled") }
    pub fn forcefield_duration() -> u64 { get_offset("SpawnLocation", "ForcefieldDuration") }
    pub fn neutral() -> u64 { get_offset("SpawnLocation", "Neutral") }
    pub fn team_color() -> u64 { get_offset("SpawnLocation", "TeamColor") }
}

// Beam (rope/tracer effects between two Attachments)
pub mod beam {
    use super::get_offset;
    pub fn attachment0() -> u64 { get_offset("Beam", "Attachment0") }
    pub fn attachment1() -> u64 { get_offset("Beam", "Attachment1") }
    pub fn brightness() -> u64 { get_offset("Beam", "Brightness") }
    pub fn curve_size0() -> u64 { get_offset("Beam", "CurveSize0") }
    pub fn curve_size1() -> u64 { get_offset("Beam", "CurveSize1") }
    pub fn light_emission() -> u64 { get_offset("Beam", "LightEmission") }
    pub fn light_influence() -> u64 { get_offset("Beam", "LightInfluence") }
    pub fn texture() -> u64 { get_offset("Beam", "Texture") }
    pub fn texture_length() -> u64 { get_offset("Beam", "TextureLength") }
    pub fn texture_speed() -> u64 { get_offset("Beam", "TextureSpeed") }
    pub fn width0() -> u64 { get_offset("Beam", "Width0") }
    pub fn width1() -> u64 { get_offset("Beam", "Width1") }
    pub fn z_offset() -> u64 { get_offset("Beam", "ZOffset") }
}

// BloomEffect
pub mod bloom_effect {
    use super::get_offset;
    pub fn enabled() -> u64 { get_offset("BloomEffect", "Enabled") }
    pub fn intensity() -> u64 { get_offset("BloomEffect", "Intensity") }
    pub fn size() -> u64 { get_offset("BloomEffect", "Size") }
    pub fn threshold() -> u64 { get_offset("BloomEffect", "Threshold") }
}

// DepthOfFieldEffect
pub mod depth_of_field_effect {
    use super::get_offset;
    pub fn enabled() -> u64 { get_offset("DepthOfFieldEffect", "Enabled") }
    pub fn far_intensity() -> u64 { get_offset("DepthOfFieldEffect", "FarIntensity") }
    pub fn focus_distance() -> u64 { get_offset("DepthOfFieldEffect", "FocusDistance") }
    pub fn in_focus_radius() -> u64 { get_offset("DepthOfFieldEffect", "InFocusRadius") }
    pub fn near_intensity() -> u64 { get_offset("DepthOfFieldEffect", "NearIntensity") }
}

// SunRaysEffect
pub mod sun_rays_effect {
    use super::get_offset;
    pub fn enabled() -> u64 { get_offset("SunRaysEffect", "Enabled") }
    pub fn intensity() -> u64 { get_offset("SunRaysEffect", "Intensity") }
    pub fn spread() -> u64 { get_offset("SunRaysEffect", "Spread") }
}

// Clothing (avatar shirt/pants item)
pub mod clothing {
    use super::get_offset;
    pub fn color3() -> u64 { get_offset("Clothing", "Color3") }
    pub fn template() -> u64 { get_offset("Clothing", "Template") }
}

// CharacterMesh (avatar body mesh overrides)
pub mod character_mesh {
    use super::get_offset;
    pub fn base_texture_id() -> u64 { get_offset("CharacterMesh", "BaseTextureId") }
    pub fn body_part() -> u64 { get_offset("CharacterMesh", "BodyPart") }
    pub fn mesh_id() -> u64 { get_offset("CharacterMesh", "MeshId") }
    pub fn overlay_texture_id() -> u64 { get_offset("CharacterMesh", "OverlayTextureId") }
}

// GuiBase2D (base for all 2D GUI elements — AbsoluteSize/Position read-only)
pub mod gui_base2d {
    use super::get_offset;
    pub fn absolute_position() -> u64 { get_offset("GuiBase2D", "AbsolutePosition") }
    pub fn absolute_rotation() -> u64 { get_offset("GuiBase2D", "AbsoluteRotation") }
    pub fn absolute_size() -> u64 { get_offset("GuiBase2D", "AbsoluteSize") }
}

// UnionOperation (CSG union — exposes AssetId for mesh lookup)
pub mod union_operation {
    use super::get_offset;
    pub fn asset_id() -> u64 { get_offset("UnionOperation", "AssetId") }
}

// Textures (Decal / Texture content pointers)
pub mod textures {
    use super::get_offset;
    pub fn decal_texture() -> u64 { get_offset("Textures", "Decal_Texture") }
    pub fn texture_texture() -> u64 { get_offset("Textures", "Texture_Texture") }
}

// ScriptContext service (executor bypass flag)
pub mod script_context {
    use super::get_offset;
    /// RequireBypass offset within the ScriptContext service instance.
    /// Dump value 0x0 — patch to non-zero to bypass require() security.
    pub fn require_bypass() -> u64 { get_offset("ScriptContext", "RequireBypass") }
}

// MeshContentProvider (global mesh cache — used by mesh CAMs)
pub mod mesh_content_provider {
    use super::get_offset;
    pub fn asset_id() -> u64 { get_offset("MeshContentProvider", "AssetID") }
    pub fn cache() -> u64 { get_offset("MeshContentProvider", "Cache") }
    pub fn lru_cache() -> u64 { get_offset("MeshContentProvider", "LRUCache") }
    pub fn mesh_data() -> u64 { get_offset("MeshContentProvider", "MeshData") }
    pub fn to_mesh_data() -> u64 { get_offset("MeshContentProvider", "ToMeshData") }
}

// MeshData (vertex/face array bounds — used by mesh CAMs)
pub mod mesh_data {
    use super::get_offset;
    pub fn vertex_start() -> u64 { get_offset("MeshData", "VertexStart") }
    pub fn vertex_end() -> u64 { get_offset("MeshData", "VertexEnd") }
    pub fn face_start() -> u64 { get_offset("MeshData", "FaceStart") }
    pub fn face_end() -> u64 { get_offset("MeshData", "FaceEnd") }
}

// Script (generic Script instance — shares ByteCode structure with LocalScript)
pub mod script {
    use super::get_offset;
    pub fn byte_code() -> u64 { get_offset("Script", "ByteCode") }
    pub fn guid() -> u64 { get_offset("Script", "GUID") }
    pub fn hash() -> u64 { get_offset("Script", "Hash") }
}

