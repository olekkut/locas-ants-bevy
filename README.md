# Locas-Ants ECS: High-Performance Ant Simulation in Rust & Bevy

A fast, cross-platform, multi-threaded ant swarm simulation built with the **Bevy Engine** utilizing the **Entity Component System (ECS)** pattern and lock-free concurrency. It simulates thousands of ants running smoothly at 60+ FPS with real-time UI controls and grid editing.

---

## 🚀 Key Features

*   **Massive Agent Counts**: Powered by Bevy ECS, utilizing parallel iteration (`par_iter_mut`) to scale efficiently on multi-core CPUs.
*   **Procedural Graphics**: Zero-asset footprint! Cells are flat color sprites, ants are custom dynamic meshes built on the GPU, and pheromones/portals are drawn procedurally using CPU-efficient Bevy Gizmos.
*   **Lock-Free Swarm Memory**: Grid cell pheromones are stored in 64-bit integers and updated concurrently via thread-safe atomic `AtomicU64` bit-packing to prevent CPU thread lock contention.
*   **Teleportation Portals**: Dual-colored portal links allow ants to transition across spatial nodes, complete with trail-path clearing.
*   **Terrain Friction**: Muddy grass cells slow ants down, simulating drag.
*   **Advanced Camera Controls**: Smooth dragging (Pan View) and dynamic scale zooming targeting the cursor coordinate.
*   **Integrated Sidebar Controller**: Runtime buttons to spawn/despawn ants, adjust simulation speed multipliers, select map-building tools, and toggle pheromone line displays.

---

## 🛠️ System Requirements & Installation

Before building, you must install the [Rust Toolchain (Cargo)](https://rustup.rs/).

### 💻 Windows
Windows supports building natively. The simulation is optimized to run on top of DirectX 12.
1. Double-click `build_and_run.bat` to automatically build and launch the simulation.
2. Alternatively, run in a terminal:
   ```powershell
   $env:WGPU_BACKEND="dx12"
   cargo run --release
   ```

### 🍎 macOS
macOS builds out of the box using Apple's Metal graphics API.
1. Execute the build shell script:
   ```bash
   chmod +x ./build_and_run.sh
   ./build_and_run.sh
   ```
2. Or run standard Cargo:
   ```bash
   cargo run --release
   ```

### 🐧 Linux
Linux requires several development packages for graphics drivers, audio interfaces, and window systems.

#### 📦 Install Dependencies
Choose the command corresponding to your Linux distribution:

*   **Ubuntu / Debian**:
    ```bash
    sudo apt-get update
    sudo apt-get install -y pkg-config libx11-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
    ```
*   **Fedora / RHEL**:
    ```bash
    sudo dnf install -y pkgconf-pkg-config libX11-devel alsa-lib-devel systemd-devel wayland-devel libxkbcommon-devel
    ```
*   **Arch Linux**:
    ```bash
    sudo pacman -S --needed pkgconf libx11 alsa-lib udev wayland libxkbcommon
    ```

#### 🏃 Launch
Run the bash script or Cargo:
```bash
chmod +x ./build_and_run.sh
./build_and_run.sh
```

---

## 🕹️ Controls & Interaction

### Sidebar UI Controls
*   **FPS & Ant Count**: Live diagnostics of the simulation.
*   **Speed Meter**: Displays Paused or the current simulation step multiplier (1x, 2x, 4x, 8x).
*   **zoom+ / zoom-**: Scales viewport zoom (also supports mouse wheel).
*   **ants+ / ants-**: Spawns or deletes 1,000 ants dynamically at the cave center.
*   **speed+ / speed-**: Speeds up, slows down, or pauses the simulation.
*   **phrms**: Toggles rendering of fading yellow/blue pheromone paths.

### Editor Drawing Tools
Select a tool in the sidebar and left-click/drag on the grid to draw:
*   `pan view`: Drag to pan around the simulation (RMB/MMB also works at any time).
*   `block`: Places impassable wall tiles.
*   `grass`: Places slow-friction terrain.
*   `cave`: Places the ants' home cave.
*   `food`: Places yellow food source cells.
*   `portal`: Click once to place a blue portal, and click again elsewhere to place its linked orange portal.
*   `remove`: Click/drag to erase cells back to empty space.

### Keyboard Shortcuts
*   `[Spacebar]`: Instantly toggle pheromone debug lines visibility.
