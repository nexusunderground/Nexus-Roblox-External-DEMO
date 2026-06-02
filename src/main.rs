// NEXUS - Roblox External Overlay
//! Edit `config.toml` to customize settings, or use the in-game menu.

/// No-op performance scope macro (demo build has no profiling).
#[macro_export]
macro_rules! perf_scope {
    ($name:expr) => {};
}

mod config;
mod core;
mod features;
mod render;
mod sdk;
mod utils;

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::env;

use crate::config::ConfigManager;
use crate::core::offsets::{fake_datamodel, visual_engine};
use crate::core::offset_loader;
use crate::core::Memory;
use crate::sdk::{Instance, VisualEngine};
use crate::utils::Cache;


#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let use_syscalls = args.iter().any(|arg| arg == "-syscall" || arg == "--syscall");
    // Overlay debug modes: glow1=default, glow2=non-transparent, glow3=nomsaa+novsync, glow4=WS_EX_TRANSPARENT
    let glow_mode1 = args.iter().any(|arg| arg.eq_ignore_ascii_case("-glow1"));
    let glow_mode2 = args.iter().any(|arg| arg.eq_ignore_ascii_case("-glow2")); // non-transparent
    let glow_mode3 = args.iter().any(|arg| arg.eq_ignore_ascii_case("-glow3")); // msaa off + vsync off
    let glow_mode4 = args.iter().any(|arg| arg.eq_ignore_ascii_case("-glow4")); // WS_EX_TRANSPARENT style
    let disable_vsync = args.iter().any(|arg| arg.eq_ignore_ascii_case("-novsync"));
    let msaa_off = args.iter().any(|arg| arg.eq_ignore_ascii_case("-nomsa") || arg.eq_ignore_ascii_case("-msaa0"));
    
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("nexus=info".parse().unwrap())
            // Suppress noisy chromiumoxide WebSocket deserialization errors
            .add_directive("chromiumoxide=off".parse().unwrap())
            .add_directive("tungstenite=off".parse().unwrap()))
        .with_target(false)
        .init();

    if use_syscalls {
        crate::core::memory::enable_syscalls();
    }

    if let Err(e) = offset_loader::initialize_offsets().await {
        eprintln!("\n❌ Failed to load offsets: {}", e);
        eprintln!("\nMake sure:");
        eprintln!("  1. Roblox is running");
        eprintln!("  2. You have internet connection");
        eprintln!("  3. The detected version has offsets available");
        eprintln!("\nPress Enter to exit...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        std::process::exit(1);
    }
    
    let discord_username = "DEMO-User".to_string(); 
    let config_manager = Arc::new(ConfigManager::new());
    let config = config_manager.get();

    tracing::info!("Username: {}", config.general.username);

    if config.general.username == "YourUsernameHere" {
        tracing::warn!("⚠ Please set your username in config.toml!");
    }

    let memory = match init_memory(&config.general.process_name) {
        Some(m) => Arc::new(m),
        None => return,
    };

    let base = memory.base_address();

    let (datamodel, visengine, players, workspace) = match init_game_instances(&memory, base) {
        Some(instances) => instances,
        None => return,
    };

    let cache = Arc::new(Cache::new());
    cache.start(Arc::clone(&players), Arc::clone(&workspace), Arc::clone(&memory), config.performance.cache_update_ms);

    run_overlay(
        cache,
        visengine,
        config_manager,
        memory,
        datamodel,
        discord_username,
        OverlayDebugFlags {
            glow_mode1,
            glow_mode2,
            glow_mode3,
            glow_mode4,
            disable_vsync,
            msaa_off,
        },
    );
}

// Initialization

fn init_memory(process_name: &str) -> Option<Memory> {
    let mut memory = Memory::new();

    if let Err(e) = memory.attach(process_name) {
        tracing::error!("Failed to attach to {}: {}", process_name, e);
        wait_and_exit();
        return None;
    }

    tracing::info!("Attached to {}", process_name);
    Some(memory)
}

fn init_game_instances(
    memory: &Arc<Memory>,
    base: u64,
) -> Option<(Arc<Instance>, Arc<VisualEngine>, Arc<Instance>, Arc<Instance>)> {
    let fake_dm = memory.read::<u64>(base + fake_datamodel::pointer());
    if fake_dm == 0 {
        tracing::error!("FakeDataModel is null");
        wait_and_exit();
        return None;
    }

    let dm_addr = memory.read::<u64>(fake_dm + fake_datamodel::real_datamodel());
    let datamodel = Arc::new(Instance::new(dm_addr, Arc::clone(memory)));

    let ve_addr = memory.read::<u64>(base + visual_engine::pointer());
    let visengine = Arc::new(VisualEngine::new(ve_addr, Arc::clone(memory)));

    let players = datamodel.find_first_child_by_class("Players")?;
    let players = Arc::new(players);

    let workspace = datamodel.find_first_child_by_class("Workspace")?;
    let workspace = Arc::new(workspace);

    tracing::info!("Game services initialized");
    Some((datamodel, visengine, players, workspace))
}

fn wait_and_exit() {
    println!("\nExiting in 5 seconds...");
    thread::sleep(Duration::from_secs(5));
}

// Overlay

struct OverlayDebugFlags {
    glow_mode1: bool,  // Default: transparent, maximized, mouse passthrough
    glow_mode2: bool,  // Non-transparent full-size (diagnose alpha issues)
    glow_mode3: bool,  // MSAA off + VSync off (fixes some NVIDIA drivers)
    glow_mode4: bool,  // Adds Windows WS_EX_TRANSPARENT click-through style
    disable_vsync: bool,
    msaa_off: bool,
}

fn run_overlay(
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    config_manager: Arc<ConfigManager>,
    memory: Arc<Memory>,
    datamodel: Arc<Instance>,
    discord_username: String,
    flags: OverlayDebugFlags,
) {
    let config = config_manager.get();

    // Build multiple debug variants
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&config.general.window_title)
        .with_decorations(false);

    // glow2: Non-transparent full-size for diagnosing black/alpha issues
    if flags.glow_mode2 {
        viewport = viewport
            .with_transparent(false)
            .with_always_on_top()
            .with_maximized(true);
    } else {
        // glow1/glow3/glow4: Transparent overlay, always on top, maximized
        viewport = viewport
            .with_transparent(true)
            .with_always_on_top()
            .with_maximized(true)
            .with_mouse_passthrough(true);
    }

    // glow3: MSAA off + VSync off (known to fix some NVIDIA driver issues)
    let (msaa, vsync) = if flags.glow_mode3 {
        (0, false)
    } else {
        (if flags.msaa_off { 0 } else { 1 }, !flags.disable_vsync)
    };

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        multisampling: msaa,
        vsync,
        ..Default::default()
    };

    let _ = eframe::run_native(
        &config.general.window_title,
        options,
        Box::new(move |cc| {
            // Log GL driver info for debugging black overlays
            if let Some(gl) = cc.gl.as_ref() {
                use eframe::glow::HasContext as _;
                unsafe {
                    let vendor = gl.get_parameter_string(eframe::glow::VENDOR);
                    let renderer = gl.get_parameter_string(eframe::glow::RENDERER);
                    let version = gl.get_parameter_string(eframe::glow::VERSION);
                    tracing::info!(target: "nexus", "OpenGL Vendor: {} | Renderer: {} | Version: {}", vendor, renderer, version);
                }
            } else {
                tracing::warn!(target: "nexus", "No GL context available at creation");
            }
            Ok(Box::new(render::OverlayApp::new(
                cache,
                visengine,
                config_manager,
                memory,
                datamodel,
                discord_username,
                flags.glow_mode4, // WS_EX_TRANSPARENT click-through style
            )))
        }),
    );
}
