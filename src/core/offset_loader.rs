#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use sysinfo::System;
use super::cloudflare_bypass;

lazy_static! {
    static ref OFFSETS: Arc<RwLock<OffsetDatabase>> = Arc::new(RwLock::new(OffsetDatabase::default()));
}

#[derive(Debug, Clone, Default)]
pub struct OffsetDatabase {
    pub version: String,
    pub dumper_version: String,
    pub dumped_at: String,
    pub total_offsets: usize,
    
    pub animation_track: HashMap<String, usize>,
    pub base_part: HashMap<String, usize>,
    pub bytecode: HashMap<String, usize>,
    pub camera: HashMap<String, usize>,
    pub click_detector: HashMap<String, usize>,
    pub datamodel: HashMap<String, usize>,
    pub fflags: HashMap<String, usize>,
    pub fake_datamodel: HashMap<String, usize>,
    pub gui_object: HashMap<String, usize>,
    pub humanoid: HashMap<String, usize>,
    pub instance: HashMap<String, usize>,
    pub lighting: HashMap<String, usize>,
    pub local_script: HashMap<String, usize>,
    pub mesh_part: HashMap<String, usize>,
    pub misc: HashMap<String, usize>,
    pub model: HashMap<String, usize>,
    pub module_script: HashMap<String, usize>,
    pub mouse_service: HashMap<String, usize>,
    pub player: HashMap<String, usize>,
    pub player_configurer: HashMap<String, usize>,
    pub player_mouse: HashMap<String, usize>,
    pub primitive: HashMap<String, usize>,
    pub primitive_flags: HashMap<String, usize>,
    pub proximity_prompt: HashMap<String, usize>,
    pub render_view: HashMap<String, usize>,
    pub run_service: HashMap<String, usize>,
    pub sky: HashMap<String, usize>,
    pub special_mesh: HashMap<String, usize>,
    pub stats_item: HashMap<String, usize>,
    pub task_scheduler: HashMap<String, usize>,
    pub team: HashMap<String, usize>,
    pub textures: HashMap<String, usize>,
    pub value_base: HashMap<String, usize>,
    pub visual_engine: HashMap<String, usize>,
    pub workspace: HashMap<String, usize>,
    pub frame: HashMap<String, usize>,
}

impl OffsetDatabase {
    pub fn get(&self, namespace: &str, name: &str) -> Option<usize> {
        let map = match namespace.to_lowercase().as_str() {
            "animationtrack" => &self.animation_track,
            "basepart" => &self.base_part,
            "bytecode" => &self.bytecode,
            "camera" => &self.camera,
            "clickdetector" => &self.click_detector,
            "datamodel" => &self.datamodel,
            "fflags" => &self.fflags,
            "fakedatamodel" => &self.fake_datamodel,
            "guiobject" => &self.gui_object,
            "humanoid" => &self.humanoid,
            "instance" => &self.instance,
            "lighting" => &self.lighting,
            "localscript" => &self.local_script,
            "meshpart" => &self.mesh_part,
            "misc" => &self.misc,
            "model" => &self.model,
            "modulescript" => &self.module_script,
            "mouseservice" => &self.mouse_service,
            "player" => &self.player,
            "playerconfigurer" => &self.player_configurer,
            "playermouse" => &self.player_mouse,
            "primitive" => &self.primitive,
            "primitiveflags" => &self.primitive_flags,
            "proximityprompt" => &self.proximity_prompt,
            "renderview" => &self.render_view,
            "runservice" => &self.run_service,
            "sky" => &self.sky,
            "specialmesh" => &self.special_mesh,
            "statsitem" => &self.stats_item,
            "taskscheduler" => &self.task_scheduler,
            "team" => &self.team,
            "textures" => &self.textures,
            "valuebase" => &self.value_base,
            "visualengine" => &self.visual_engine,
            "workspace" => &self.workspace,
            "frame" => &self.frame,
            _ => return None,
        };
        
        map.get(name).copied()
    }
}

pub fn detect_roblox_version() -> Result<String> {
    let mut system = System::new_all();
    system.refresh_all();

    let version_regex = Regex::new(r"version-([a-f0-9]+)").unwrap();

    for (_, process) in system.processes() {
        let process_name = process.name().to_lowercase();
        
        if process_name.contains("robloxplayerbeta") || process_name.contains("roblox") {
            if let Some(exe_path) = process.exe() {
                if let Some(path_str) = exe_path.to_str() {
                    if let Some(captures) = version_regex.captures(path_str) {
                        let version = format!("version-{}", &captures[1]);
                        tracing::debug!("Detected Roblox version: {} ({})", version, path_str);
                        return Ok(version);
                    }
                }
            }
        }
    }

    Err(anyhow!("Roblox process not found. Please ensure Roblox is running."))
}

/// Check if response content is a Cloudflare challenge page
fn is_cloudflare_challenge(content: &str) -> bool {
    content.contains("challenge-platform") || 
    content.contains("cf-browser-verification") ||
    content.contains("Just a moment") ||
    content.contains("Checking your browser") ||
    (content.contains("cloudflare") && content.contains("challenge")) ||
    // Also check if it looks like HTML when we expect C++ headers
    (content.contains("<!DOCTYPE") || content.contains("<html")) && !content.contains("namespace")
}

pub async fn download_offsets(version: &str) -> Result<String> {
    let url = format!("https://imtheo.lol/Offsets/{}/Offsets.hpp", version);
    
    tracing::debug!("Downloading offsets from primary source: {}", url);
    
    // First, try a simple HTTP request (might work if no Cloudflare)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36")
        .build()?;
    
    // Try with cached cookie first
    if let Some(cache) = cloudflare_bypass::load_cached_cookie("imtheo.lol") {
        tracing::info!("Trying with cached Cloudflare cookie...");
        
        let response = client
            .get(&url)
            .header("Cookie", format!("cf_clearance={}", cache.cf_clearance))
            .header("Accept", "text/html,application/xhtml+xml,*/*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .context("Failed to download offsets")?;
        
        if response.status().is_success() {
            let content = response.text().await?;
            
            // Check if we got actual content
            if !is_cloudflare_challenge(&content) && 
               (content.contains("namespace") || content.contains("constexpr") || content.contains("uintptr_t")) {
                tracing::debug!("Downloaded offsets using cached cookie ({} bytes)", content.len());
                return Ok(content);
            }
        }
        
        tracing::info!("Cached cookie didn't work, trying direct request...");
    }
    
    // Try direct request without cookie
    let response = client
        .get(&url)
        .header("Accept", "text/html,application/xhtml+xml,*/*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Failed to download offsets")?;
    
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download offsets: HTTP {}. This version may not be available yet.",
            response.status()
        ));
    }
    
    let content = response.text().await?;
    
    // Check if we got a Cloudflare challenge
    if is_cloudflare_challenge(&content) {
        tracing::warn!("[SOURCE 1] Cloudflare protection detected! Starting browser bypass...");
        
        // Use the browser-based bypass
        match cloudflare_bypass::download_with_cloudflare_bypass(&url).await {
            Ok(content) => {
                tracing::info!("Successfully downloaded offsets via browser ({} bytes)", content.len());
                return Ok(content);
            }
            Err(e) => {
                tracing::warn!("[SOURCE 1] Browser bypass failed: {}", e);
                return Err(anyhow!("Cloudflare bypass failed: {}", e));
            }
        }
    }
    
    // Validate the content looks like valid C++ offsets
    if !content.contains("namespace") && !content.contains("constexpr") && !content.contains("uintptr_t") {
        tracing::warn!("[SOURCE 1] Response doesn't appear to be valid offset data");
        return Err(anyhow!("Invalid response from imtheo.lol - not valid offset data"));
    }
    
    tracing::debug!("Downloaded primary offsets ({} bytes)", content.len());
    
    Ok(content)
}

pub async fn download_offsets_secondary() -> Result<String> {
    let url = "https://offsets.ntgetwritewatch.workers.dev/offsets.hpp";
    
    tracing::debug!("Downloading offsets from secondary source: {}", url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to download secondary offsets")?;
    
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download secondary offsets: HTTP {}",
            response.status()
        ));
    }
    
    let content = response.text().await?;
    
    tracing::debug!("Downloaded secondary offsets ({} bytes)", content.len());
    
    Ok(content)
}

pub fn parse_flat_offsets(content: &str, db: &mut OffsetDatabase) -> Result<usize> {
    let offset_regex = Regex::new(r"inline\s+constexpr\s+uintptr_t\s+(\w+)\s*=\s*0x([0-9a-fA-F]+);").unwrap();
    let version_regex = Regex::new(r"Roblox Version:\s*(version-[a-f0-9]+)").unwrap();
    
    if let Some(captures) = version_regex.captures(content) {
        if db.version.is_empty() {
            db.version = captures[1].to_string();
        }
    }
    
    let mut count = 0;
    
    let name_mappings: HashMap<&str, (&str, &str)> = [
        // MouseService
        ("InputObject", ("mouseservice", "InputObject")),
        ("MousePosition", ("mouseservice", "MousePosition")),
        ("MouseSensitivity", ("mouseservice", "SensitivityPointer")),
        
        // Frame/GUI
        ("FramePositionOffsetX", ("frame", "PositionX")),
        ("FramePositionOffsetY", ("frame", "PositionY")),
        ("FramePositionX", ("guiobject", "PositionX")),
        ("FramePositionY", ("guiobject", "PositionY")),
        ("FrameSizeX", ("guiobject", "SizeX")),
        ("FrameSizeY", ("guiobject", "SizeY")),
        ("FrameSizeOffsetX", ("guiobject", "SizeOffsetX")),
        ("FrameSizeOffsetY", ("guiobject", "SizeOffsetY")),
        ("FrameRotation", ("guiobject", "Rotation")),
        ("FrameVisible", ("guiobject", "Visible")),
        ("ScreenGuiEnabled", ("guiobject", "ScreenGuiEnabled")),
        ("TextLabelText", ("guiobject", "TextLabelText")),
        ("TextLabelVisible", ("guiobject", "TextLabelVisible")),
        
        // Player
        ("PlayerMouse", ("player", "Mouse")),
        ("LocalPlayer", ("player", "LocalPlayer")),
        ("UserId", ("player", "UserId")),
        ("Team", ("player", "Team")),
        ("TeamColor", ("player", "TeamColor")),
        ("DisplayName", ("player", "DisplayName")),
        ("ModelInstance", ("player", "ModelInstance")),
        ("CharacterAppearanceId", ("player", "CharacterAppearanceId")),
        ("CameraMaxZoomDistance", ("player", "MaxZoomDistance")),
        ("CameraMinZoomDistance", ("player", "MinZoomDistance")),
        ("CameraMode", ("player", "CameraMode")),
        
        // Camera
        ("Camera", ("workspace", "CurrentCamera")),
        ("CameraPos", ("camera", "Position")),
        ("CameraRotation", ("camera", "Rotation")),
        ("CameraSubject", ("camera", "CameraSubject")),
        ("CameraType", ("camera", "CameraType")),
        ("FOV", ("camera", "FieldOfView")),
        ("ViewportSize", ("camera", "ViewportSize")),
        
        // Instance
        ("Name", ("instance", "Name")),
        ("NameSize", ("instance", "NameSize")),
        ("Parent", ("instance", "Parent")),
        ("Children", ("instance", "ChildrenStart")),
        ("ChildrenEnd", ("instance", "ChildrenEnd")),
        ("ClassDescriptor", ("instance", "ClassDescriptor")),
        ("ClassDescriptorToClassName", ("instance", "ClassName")),
        ("InstanceAttributePointer1", ("instance", "AttributeContainer")),
        ("InstanceAttributePointer2", ("instance", "AttributeList")),
        ("AttributeToNext", ("instance", "AttributeToNext")),
        ("AttributeToValue", ("instance", "AttributeToValue")),
        ("OnDemandInstance", ("instance", "OnDemandInstance")),
        ("InstanceCapabilities", ("instance", "Capabilities")),
        
        // FakeDataModel/DataModel
        ("FakeDataModelPointer", ("fakedatamodel", "Pointer")),
        ("FakeDataModelToDataModel", ("fakedatamodel", "RealDataModel")),
        ("Workspace", ("datamodel", "Workspace")),
        ("PlaceId", ("datamodel", "PlaceId")),
        ("GameId", ("datamodel", "GameId")),
        ("CreatorId", ("datamodel", "CreatorId")),
        ("JobId", ("datamodel", "JobId")),
        ("ScriptContext", ("datamodel", "ScriptContext")),
        ("GameLoaded", ("datamodel", "GameLoaded")),
        ("DataModelPrimitiveCount", ("datamodel", "PrimitiveCount")),
        ("DataModelDeleterPointer", ("datamodel", "DeleterPointer")),
        ("DataModelToRenderView1", ("datamodel", "ToRenderView1")),
        ("DataModelToRenderView2", ("datamodel", "ToRenderView2")),
        ("DataModelToRenderView3", ("datamodel", "ToRenderView3")),
        
        // VisualEngine
        ("VisualEnginePointer", ("visualengine", "Pointer")),
        ("Dimensions", ("visualengine", "Dimensions")),
        ("viewmatrix", ("visualengine", "ViewMatrix")),
        ("VisualEngineToDataModel1", ("visualengine", "ToDataModel1")),
        ("VisualEngineToDataModel2", ("visualengine", "ToDataModel2")),
        ("VisualEngine", ("visualengine", "VisualEngine")),
        
        // Humanoid
        ("Health", ("humanoid", "Health")),
        ("MaxHealth", ("humanoid", "MaxHealth")),
        ("WalkSpeed", ("humanoid", "WalkSpeed")),
        ("JumpPower", ("humanoid", "JumpPower")),
        ("HipHeight", ("humanoid", "HipHeight")),
        ("RigType", ("humanoid", "RigType")),
        ("MoveDirection", ("humanoid", "MoveDirection")),
        ("RootPartR15", ("humanoid", "RootPartR15")),
        ("RootPartR6", ("humanoid", "RootPartR6")),
        ("MaxSlopeAngle", ("humanoid", "MaxSlopeAngle")),
        ("HumanoidDisplayName", ("humanoid", "DisplayName")),
        ("HumanoidState", ("humanoid", "State")),
        ("HumanoidStateId", ("humanoid", "StateId")),
        ("HealthDisplayDistance", ("humanoid", "HealthDisplayDistance")),
        ("NameDisplayDistance", ("humanoid", "NameDisplayDistance")),
        ("Sit", ("humanoid", "Sit")),
        ("EvaluateStateMachine", ("humanoid", "EvaluateStateMachine")),
        ("AutoJumpEnabled", ("humanoid", "AutoJumpEnabled")),
        
        // BasePart
        ("CFrame", ("basepart", "CFrame")),
        ("Position", ("basepart", "Position")),
        ("Rotation", ("basepart", "Rotation")),
        ("PartSize", ("basepart", "Size")),
        ("Velocity", ("basepart", "Velocity")),
        ("Transparency", ("basepart", "Transparency")),
        ("Primitive", ("basepart", "Primitive")),
        ("Anchored", ("basepart", "Anchored")),
        ("AnchoredMask", ("basepart", "AnchoredMask")),
        ("CanCollide", ("basepart", "CanCollide")),
        ("CanCollideMask", ("basepart", "CanCollideMask")),
        ("CanTouch", ("basepart", "CanTouch")),
        ("CanTouchMask", ("basepart", "CanTouchMask")),
        ("MaterialType", ("basepart", "MaterialType")),
        
        // Primitive
        ("PrimitivesPointer1", ("primitive", "Pointer1")),
        ("PrimitivesPointer2", ("primitive", "Pointer2")),
        ("PrimitiveValidateValue", ("primitive", "ValidateValue")),
        
        // TaskScheduler
        ("TaskSchedulerPointer", ("taskscheduler", "Pointer")),
        ("TaskSchedulerMaxFPS", ("taskscheduler", "MaxFPS")),
        ("JobStart", ("taskscheduler", "JobStart")),
        ("JobEnd", ("taskscheduler", "JobEnd")),
        ("JobsPointer", ("taskscheduler", "JobsPointer")),
        ("Job_Name", ("taskscheduler", "JobName")),
        
        // Workspace
        ("Gravity", ("workspace", "Gravity")),
        ("WorkspaceToWorld", ("workspace", "ToWorld")),
        
        // ValueBase
        ("Value", ("valuebase", "Value")),
        ("ValueGetSetToValue", ("valuebase", "GetSetToValue")),
        
        // Lighting
        ("ClockTime", ("lighting", "ClockTime")),
        ("FogColor", ("lighting", "FogColor")),
        ("FogStart", ("lighting", "FogStart")),
        ("FogEnd", ("lighting", "FogEnd")),
        ("OutdoorAmbient", ("lighting", "OutdoorAmbient")),
        
        // Sky
        ("SkyboxBk", ("sky", "SkyboxBk")),
        ("SkyboxDn", ("sky", "SkyboxDn")),
        ("SkyboxFt", ("sky", "SkyboxFt")),
        ("SkyboxLf", ("sky", "SkyboxLf")),
        ("SkyboxRt", ("sky", "SkyboxRt")),
        ("SkyboxUp", ("sky", "SkyboxUp")),
        ("MoonTextureId", ("sky", "MoonTextureId")),
        ("SunTextureId", ("sky", "SunTextureId")),
        ("StarCount", ("sky", "StarCount")),
        
        // ClickDetector
        ("ClickDetectorMaxActivationDistance", ("clickdetector", "MaxActivationDistance")),
        
        // ProximityPrompt
        ("ProximityPromptEnabled", ("proximityprompt", "Enabled")),
        ("ProximityPromptMaxActivationDistance", ("proximityprompt", "MaxActivationDistance")),
        ("ProximityPromptHoldDuraction", ("proximityprompt", "HoldDuration")),
        ("ProximityPromptActionText", ("proximityprompt", "ActionText")),
        ("ProximityPromptMaxObjectText", ("proximityprompt", "ObjectText")),
        ("ProximityPromptGamepadKeyCode", ("proximityprompt", "GamepadKeyCode")),
        
        // LocalScript/ModuleScript (Bytecode)
        ("LocalScriptByteCode", ("localscript", "ByteCode")),
        ("LocalScriptBytecodePointer", ("bytecode", "LocalScriptPointer")),
        ("LocalScriptHash", ("localscript", "Hash")),
        ("ModuleScriptByteCode", ("modulescript", "ByteCode")),
        ("ModuleScriptBytecodePointer", ("bytecode", "ModuleScriptPointer")),
        ("ModuleScriptHash", ("modulescript", "Hash")),
        ("RunContext", ("localscript", "RunContext")),
        ("Sandboxed", ("localscript", "Sandboxed")),
        
        // MeshPart
        ("MeshPartColor3", ("meshpart", "Color3")),
        ("MeshPartTexture", ("meshpart", "Texture")),
        
        // Misc
        ("Adornee", ("misc", "Adornee")),
        ("AnimationId", ("animationtrack", "AnimationId")),
        ("DecalTexture", ("textures", "DecalTexture")),
        ("SoundId", ("misc", "SoundId")),
        ("Ping", ("misc", "Ping")),
        ("Tool_Grip_Position", ("misc", "ToolGripPosition")),
        ("Deleter", ("misc", "Deleter")),
        ("DeleterBack", ("misc", "DeleterBack")),
        ("StringLength", ("misc", "StringLength")),
        
        // FFlags
        ("FFlagList", ("fflags", "List")),
        ("FFlagToValueGetSet", ("fflags", "ToValueGetSet")),
        
        // PlayerConfigurer
        ("PlayerConfigurerPointer", ("playerconfigurer", "Pointer")),
        ("BanningEnabled", ("playerconfigurer", "BanningEnabled")),
        ("ForceNewAFKDuration", ("playerconfigurer", "ForceNewAFKDuration")),
        
        // RenderView
        ("RenderJobToDataModel", ("renderview", "ToDataModel")),
        ("RenderJobToFakeDataModel", ("renderview", "ToFakeDataModel")),
        ("RenderJobToRenderView", ("renderview", "ToRenderView")),
        ("InsetMinX", ("renderview", "InsetMinX")),
        ("InsetMinY", ("renderview", "InsetMinY")),
        ("InsetMaxX", ("renderview", "InsetMaxX")),
        ("InsetMaxY", ("renderview", "InsetMaxY")),
        
        // Beam effects
        ("BeamBrightness", ("misc", "BeamBrightness")),
        ("BeamColor", ("misc", "BeamColor")),
        ("BeamLightEmission", ("misc", "BeamLightEmission")),
        ("BeamLightInfuence", ("misc", "BeamLightInfluence")),
    ].iter().cloned().collect();
    
    // Additional mappings where one source name maps to multiple destinations
    // Velocity is used on both BasePart and Primitive (same offset value)
    let multi_mappings: HashMap<&str, Vec<(&str, &str)>> = [
        ("Velocity", vec![("primitive", "Velocity")]),
        ("Position", vec![("primitive", "Position")]),
    ].iter().cloned().collect();
    
    for line in content.lines() {
        if let Some(captures) = offset_regex.captures(line) {
            let name = &captures[1];
            let value = usize::from_str_radix(&captures[2], 16)
                .context(format!("Failed to parse hex value: {}", &captures[2]))?;
            
            if let Some((namespace, mapped_name)) = name_mappings.get(name) {
                let map = match *namespace {
                    "mouseservice" => &mut db.mouse_service,
                    "player" => &mut db.player,
                    "camera" => &mut db.camera,
                    "instance" => &mut db.instance,
                    "fakedatamodel" => &mut db.fake_datamodel,
                    "datamodel" => &mut db.datamodel,
                    "visualengine" => &mut db.visual_engine,
                    "humanoid" => &mut db.humanoid,
                    "basepart" => &mut db.base_part,
                    "taskscheduler" => &mut db.task_scheduler,
                    "valuebase" => &mut db.value_base,
                    "workspace" => &mut db.workspace,
                    "frame" => &mut db.frame,
                    _ => continue,
                };
                
                let existing = map.get(*mapped_name).copied();
                let should_insert = match existing {
                    None => true,
                    Some(0) => value != 0,
                    Some(_) => false,
                };
                
                if should_insert && value != 0 {
                    map.insert(mapped_name.to_string(), value);
                    count += 1;
                }
            }
            
            // Handle multi-mappings (same source offset to multiple destinations)
            if let Some(mappings) = multi_mappings.get(name) {
                for (namespace, mapped_name) in mappings {
                    let map = match *namespace {
                        "primitive" => &mut db.primitive,
                        "mouseservice" => &mut db.mouse_service,
                        "player" => &mut db.player,
                        "camera" => &mut db.camera,
                        "instance" => &mut db.instance,
                        "fakedatamodel" => &mut db.fake_datamodel,
                        "datamodel" => &mut db.datamodel,
                        "visualengine" => &mut db.visual_engine,
                        "humanoid" => &mut db.humanoid,
                        "basepart" => &mut db.base_part,
                        "taskscheduler" => &mut db.task_scheduler,
                        "valuebase" => &mut db.value_base,
                        "workspace" => &mut db.workspace,
                        "frame" => &mut db.frame,
                        _ => continue,
                    };
                    
                    let existing = map.get(*mapped_name).copied();
                    let should_insert = match existing {
                        None => true,
                        Some(0) => value != 0,
                        Some(_) => false,
                    };
                    
                    if should_insert && value != 0 {
                        map.insert(mapped_name.to_string(), value);
                        count += 1;
                    }
                }
            }
        }
    }
    
    Ok(count)
}

pub fn parse_cpp_offsets(content: &str) -> Result<OffsetDatabase> {
    let mut db = OffsetDatabase::default();
    
    let version_regex = Regex::new(r#"ClientVersion = "(.+?)""#).unwrap();
    let namespace_regex = Regex::new(r"namespace\s+(\w+)\s*\{").unwrap();
    let offset_regex = Regex::new(r"inline\s+constexpr\s+uintptr_t\s+(\w+)\s*=\s*0x([0-9a-fA-F]+);").unwrap();
    
    if let Some(captures) = version_regex.captures(content) {
        db.version = captures[1].to_string();
    }
    
    let mut current_namespace = String::new();
    let mut brace_count = 0;
    
    for line in content.lines() {
        let trimmed = line.trim();
        
        if let Some(captures) = namespace_regex.captures(trimmed) {
            current_namespace = captures[1].to_string();
            brace_count = 1;
            continue;
        }
        
        brace_count += trimmed.matches('{').count() as i32;
        brace_count -= trimmed.matches('}').count() as i32;
        
        if brace_count <= 0 {
            current_namespace.clear();
        }
        
        if !current_namespace.is_empty() {
            if let Some(captures) = offset_regex.captures(trimmed) {
                let name = captures[1].to_string();
                let value = usize::from_str_radix(&captures[2], 16)
                    .context(format!("Failed to parse hex value: {}", &captures[2]))?;
                
                db.total_offsets += 1;
                
                let map = match current_namespace.to_lowercase().as_str() {
                    "animationtrack" => &mut db.animation_track,
                    "basepart" => &mut db.base_part,
                    "bytecode" => &mut db.bytecode,
                    "camera" => &mut db.camera,
                    "clickdetector" => &mut db.click_detector,
                    "datamodel" => &mut db.datamodel,
                    "fflags" => &mut db.fflags,
                    "fakedatamodel" => &mut db.fake_datamodel,
                    "guiobject" => &mut db.gui_object,
                    "humanoid" => &mut db.humanoid,
                    "instance" => &mut db.instance,
                    "lighting" => &mut db.lighting,
                    "localscript" => &mut db.local_script,
                    "meshpart" => &mut db.mesh_part,
                    "misc" => &mut db.misc,
                    "model" => &mut db.model,
                    "modulescript" => &mut db.module_script,
                    "mouseservice" => &mut db.mouse_service,
                    "player" => &mut db.player,
                    "playerconfigurer" => &mut db.player_configurer,
                    "playermouse" => &mut db.player_mouse,
                    "primitive" => &mut db.primitive,
                    "primitiveflags" => &mut db.primitive_flags,
                    "proximityprompt" => &mut db.proximity_prompt,
                    "renderview" => &mut db.render_view,
                    "runservice" => &mut db.run_service,
                    "sky" => &mut db.sky,
                    "specialmesh" => &mut db.special_mesh,
                    "statsitem" => &mut db.stats_item,
                    "taskscheduler" => &mut db.task_scheduler,
                    "team" => &mut db.team,
                    "textures" => &mut db.textures,
                    "valuebase" => &mut db.value_base,
                    "visualengine" => &mut db.visual_engine,
                    "workspace" => &mut db.workspace,
                    _ => {
                        tracing::debug!("Unknown namespace: {}", current_namespace);
                        continue;
                    }
                };
                
                map.insert(name, value);
            }
        }
    }
    
    tracing::debug!("Parsed {} offsets across {} namespaces", db.total_offsets, 
        [
            &db.animation_track, &db.base_part, &db.camera, &db.datamodel,
            &db.humanoid, &db.instance, &db.player, &db.workspace,
        ].iter().filter(|m| !m.is_empty()).count()
    );
    
    Ok(db)
}

pub async fn initialize_offsets() -> Result<()> {
    let version = detect_roblox_version()?;
    
    let mut db = match download_offsets(&version).await {
        Ok(content) => {
            match parse_cpp_offsets(&content) {
                Ok(parsed) => parsed,
                Err(e) => {
                    tracing::warn!("Failed to parse primary offsets: {}", e);
                    OffsetDatabase::default()
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to download primary offsets: {}", e);
            OffsetDatabase::default()
        }
    };
    
    match download_offsets_secondary().await {
        Ok(content) => {
            match parse_flat_offsets(&content, &mut db) {
                Ok(count) => { db.total_offsets += count; }
                Err(e) => { tracing::warn!("Failed to parse secondary offsets: {}", e); }
            }
        }
        Err(e) => { tracing::warn!("Failed to download secondary offsets: {}", e); }
    }
    
    if db.version.is_empty() {
        db.version = version.clone();
    }
    
    let mut placeholder_count = 0;
    let check_map = |map: &HashMap<String, usize>| -> usize {
        map.iter().filter(|(_, val)| **val == 0).count()
    };
    
    placeholder_count += check_map(&db.mouse_service);
    placeholder_count += check_map(&db.player);
    placeholder_count += check_map(&db.player_mouse);
    placeholder_count += check_map(&db.camera);
    placeholder_count += check_map(&db.humanoid);
    
    if placeholder_count > 0 {
        tracing::debug!("{} offsets have placeholder values", placeholder_count);
    }
    
    {
        let mut global = OFFSETS.write().unwrap();
        *global = db;
    }
    
    tracing::info!("Offsets loaded ({} for {})", OFFSETS.read().unwrap().total_offsets, version);
    
    Ok(())
}

pub fn get_offsets() -> Arc<RwLock<OffsetDatabase>> {
    OFFSETS.clone()
}

pub fn get_offset(namespace: &str, name: &str) -> Option<usize> {
    let db = OFFSETS.read().unwrap();
    db.get(namespace, name)
}

pub fn get_version() -> String {
    let db = OFFSETS.read().unwrap();
    db.version.clone()
}
