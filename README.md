# Nexus Underground [Demo Version]

A Rust-based external overlay tool with a modern GUI interface built using egui.

## Debug Modes (Glow) - For NVIDIA/GPU Transparency Issues

If you have a **black overlay** or **click-through not working**, try these modes:

| Flag       | Description                                    | When to Use |
|------      |-------------                                   |-------------|
| `-glow1`   | Default: transparent, maximized, click-through | Works on most systems |
| `-glow2`   | Non-transparent full-size overlay              | Diagnose alpha/clear issues |
| `-glow3`   | Transparent + MSAA off + VSync off             | **Fixes most NVIDIA issues** |
| `-glow4`   | Adds Windows `WS_EX_TRANSPARENT` style         | Extra click-through reliability |
| `-novsync` | Disable VSync (can combine with others)        | If overlay flickers |
| `-msaa0`   | Disable MSAA (can combine with others)         | If overlay is black |

### Quick Start Guide

**If default works (most systems):**
```powershell
cargo run --release
```

**If overlay is BLACK on NVIDIA GPU, try this first:**
```powershell
cargo run --release -- -glow3
```

**If still black, try:**
```powershell
cargo run --release -- -glow3 -glow4
```

**Other combinations to test:**
```powershell
cargo run --release -- -glow1 -novsync -msaa0
cargo run --release -- -glow4
```

---

## Active Features

### Visuals
- **Box ESP** - 2D bounding boxes around players
  - Distance-based coloring: Green (0-30m), Teal (30-80m), Yellow (80-150m), Red (150m+)
  - Health and armor bars
  - Team highlighting (blue), target highlighting (magenta on current aim target)
  - Configurable box style, fill, and custom colors
- **Name Tags** - Player names with optional distance label
- **Tracers** - Lines from screen edge to player positions
- **Chams Glow** - Player outline/glow effect (no mesh CDN required)
- **FOV Changer** - Camera field of view (1–120°)
- **Distance Colors** - Automatic color scaling by range
- **Target Highlight** - Magenta box on the currently locked aim target

#### ESP Filters
- **Team Check** - Whitelist system; add teammate names to skip them in ESP and aimbot
- **Hide Dead** - Suppress dead players from the overlay
- **Show Bots/NPCs** - Include non-player entities
- **Max Distance** - Configurable render cutoff (50–1000 m)

---

### Aiming

All aim systems share a global **velocity prediction** toggle with configurable lead time.
Hold **RMB** to activate aim systems.

| System | Description |
|--------|-------------|
| **Aim Assist** | FOV-guided smoothed mouse guidance. Configurable FOV, smoothing, bone target, activation mode (hold/toggle), and hold delay. |
| **Mouse Aim** | Raw `SendInput` cursor movement. Configurable FOV, smoothing, and bone target. |
| **Camera Aim** | Spoofs the Roblox camera CFrame rotation directly. Configurable FOV and bone target. |
| **Viewport Aim** | Writes a target offset into the camera viewport CFrame. Configurable FOV. |

All four systems display an optional **FOV circle** on screen.

- **Auto Reload** - Automatically reloads weapons when the magazine is empty

---

### Movement
- **Walk Speed** - Adjustable humanoid walk speed (16–500)
- **Jump Power** - Configurable jump height (50–300)
- **Auto-Jump** - Continuously jumps while grounded
- **Spinbot** - Continuous yaw rotation; configurable speed (1–30°)

---

### World
- **Anti-Fog** - Pushes fog start/end distances to eliminate fog rendering
- **Brightness** - Overrides scene ambient brightness
- **Anti-Flash** - Clamps maximum brightness to prevent flashbang-style effects

---

### Auto-Clicker
- Record any mouse button sequence (LMB, RMB, side buttons, etc.)
- Configurable delay, variance, and hold duration per click
- Turbo mode (no delays)
- Press **F10** to start/stop while in-game

---

### Hitbox Modifier
- Expands or shrinks hitbox primitive sizes on all players
- Visual wireframe preview in-overlay

---

### Utility
- **Hotkey System** - 10 fully customizable hotkey slots (bind any key to any feature)
- **Hotkey Hints HUD** - Floating widget showing active binds and toggle states
- **Configuration** - TOML-based config; auto-saves on exit, manual save with `End`
- **Cache System** - Background thread tracking all players with velocity history
- **Custom Accent Color** - RGB accent picker for the menu UI


---

## Setup Guide

**Requirements:** Windows 10 or 11, ~6 GB free disk space, internet connection

**Step 1: Install Rust**

Go to https://rust-lang.org/tools/install, download and run `rustup-init.exe`, press Enter to accept defaults, then restart.

Verify: `rustc --version`

**Step 2: Build**

```powershell
cd "C:\path\to\Nexus (demo)"
cargo build --release
```

First build takes 5-10 minutes. Subsequent builds are much faster.

**Step 3: Run**

```powershell
cargo run --release
```

Or double-click `target\release\nexus.exe`

---

## Configuration

A `config.toml` is created next to the executable on first run. Set your Roblox username so you are excluded from ESP and aimbot targeting, then restart.

---

## Hotkeys

| Key    | Function      | Notes |
|--------|---------------|-------|
| F1     | Toggle Menu   | |
| F2     | Box ESP       | |
| F3     | Chams Glow    | |
| F4     | Aim Assist    | |
| F5     | Camera Aim    | |
| F7     | Tracers       | |
| F10    | Auto-Clicker  | Start/stop |
| F11    | Hitbox Mod    | |
| F12    | Exit          | |
| Home   | Refresh       | Full game data refresh |
| End    | Save Config   | |

All 10 hotkey slots are fully customizable in the **Hotkeys** tab.

---

## Join Our Community

**Discord:** https:/tr.ee/NexusD
**Get the full version with all premium features!**

---

## License

For educational and research purposes only.

---

*"My crime is that of curiosity"* - NexusUnderground
