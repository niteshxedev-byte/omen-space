# OMEN Space Modifications & Custom Hardware Integration

This document outlines the custom hardware adaptations and modifications implemented in this repository fork for HP Victus and OMEN laptops that lack standard WMI fan control or `pwm1` sysfs nodes.

---

## Target Hardware & System Environment

- **Laptop Model:** HP Victus by HP Gaming Laptop 15-fa0xxx
- **Motherboard / Board ID:** `8A4F` (BIOS Version: `F.28`)
- **Processor:** 12th Gen Intel Core i7-12650H (16 threads)
- **Graphics:** NVIDIA GeForce RTX 3050 Mobile (4GB) + Intel UHD Graphics
- **Operating System:** Fedora Linux 43 (KDE Plasma 6, Wayland session)
- **Kernel:** `7.1.10-100.fc43.x86_64`

---

## 1. Problem Statement: HP Victus 15 Fan Control

On HP Victus 15 (`8A4F` and related boards):
1. **WMI Writes Unsupported:** Direct HP WMI fan speed writes (`0x2E` WMI command calls) are rejected by the BIOS or fail silently.
2. **Missing PWM Node:** In `/sys/class/hwmon/hwmon*` (under `hp-wmi`), only RPM tachometer readbacks (`fan1_input`, `fan2_input`) and `pwm1_enable` exist; no `pwm1` manual duty cycle file is provided by the kernel driver.
3. As a result, out-of-the-box OMEN Space disabled custom fan curves, restricted fan modes, and could not adjust the physical fan speeds.

---

## 2. Implemented Modifications

### A. NoteBook FanControl (`nbfc`) EC Integration
We integrated the NoteBook FanControl (`nbfc`) service as an Embedded Controller (EC) execution backend in `omen-space-daemon`:

- **Detection (`nbfc_available`):** Checks if `/usr/bin/nbfc` is present on the system.
- **Speed Control (`nbfc_set_speed`):** Calls `/usr/bin/nbfc set -s <pct>` to set manual speed directly on both fans via EC registers (`0x2D` and `0x2C`).
- **Auto Mode (`nbfc_set_auto`):** Calls `/usr/bin/nbfc set -a` to release fan control back to the automatic profile curve.
- **Pipeline Integration:**
  - `FanService::write_pwm_duty`: When `nbfc` is available, bypasses the strict `pwm1` sysfs file requirement and routes target duty percentages directly through `nbfc_set_speed`.
  - `FanService::set_mode_internal`:
    - Mode `max`: Sets `nbfc_set_speed(100)` (full 6000 RPM boost).
    - Mode `auto`: Dynamically evaluates CPU/GPU temperature spline curves every second and sends smoothed targets to `nbfc`.
    - Mode `ec`: Calls `nbfc_set_auto()` to delegate cooling entirely to firmware/EC profiles.
    - Mode `custom`: Evaluates user-drawn custom curves from the GUI or CLI targets and routes to `nbfc`.
  - `FanService::get_fan_info` & `has_pwm_fallback`: Reports custom curve support as `true` when `nbfc` is active.

### B. Board Database (`boards.json`)
Updated configuration for Board ID `8A4F`:
- `supports_fan_control_ec`: `true`
- `supports_fan_curves`: `true`
- `supports_rpm_readback`: `true`
- `fan_zone_count`: `2`

### C. CLI Enhancements (`omen-cli`)
Added a dedicated command to set arbitrary fan percentages:
```bash
omen-cli fan set-speed <0-100>
```
Example: `omen-cli fan set-speed 50` sets both fans to 50% target speed.

### D. Physical OMEN Key Keyboard Shortcut
Configured the physical keyboard OMEN key (key code 148 / `KEY_PROG1`) to launch OMEN Space instantly (replicating Windows behavior):
1. **Daemon Listener:** `hotkey_monitor` monitors `/dev/input/event3` and `/dev/input/event8` (HP WMI Hotkeys) and broadcasts `MacroKeyPressed("omen")` over D-Bus.
2. **Tray Service:** `omen-tray` intercepts the D-Bus signal and executes `omen-gui`.
3. **KDE Plasma Shortcut:** Registered `Launch (2)` and `Tools` in `~/.config/kglobalshortcutsrc` for `org.hp.OmenSpace.desktop`.
4. **Systemd User Unit:** Created `~/.config/systemd/user/omen-tray.service` to keep the tray applet and hotkey listener persistent across login sessions.

### E. Inactivity & Sleep Guard ("Are You Awake?")
Added a configurable inactivity timer feature to automatically detect user absence, prompt with an "Are you awake?" dialog, and safely shut down the laptop to prevent unnecessary battery drain and overheating:
1. **Hardware Evdev Monitoring (`inactivity_monitor.rs`):**
   - Directly monitors all input devices (`/dev/input/event*`) at the root daemon level for keyboards, touchpads, and mice.
   - Zero compositor dependencies (works identically on Wayland, X11, or lock screen).
   - Dynamically filters for key presses (`EV_KEY`), relative mouse movements (`EV_REL`), and absolute touchpad touches/swipes (`EV_ABS`), resetting the idle timer on real physical input.
2. **Watchdog & D-Bus IPC:**
   - Evaluates idle time every second against the configured threshold (default: 2 hours / 7200s, selectable from 30m to 4h).
   - When idle time exceeds the threshold, broadcasts `MacroKeyPressed("awake_prompt")` across D-Bus.
   - Includes a 75-second safety fallback: if the dialog is ignored or unhandled, the daemon invokes `systemctl poweroff` directly.
   - Exposed D-Bus methods: `get_inactivity_state`, `set_inactivity_timer`, `extend_inactivity_timer`, `poweroff_system`, `trigger_awake_prompt`.
3. **Interactive "Are You Awake?" Dialog (`omen-overlay`):**
   - Launched via `omen-overlay --awake-dialog` triggered by `omen-tray`.
   - Features a 60-second animated visual countdown.
   - Two clear action choices:
     - **"Yes, I'm awake (+1h to 3h)"**: Increases the timer threshold to 3 hours (10,800 seconds), resets the inactivity tracker, sends a desktop confirmation notification, and closes the dialog.
     - **"Shut Down Now"** (or countdown reaching 0): Immediately issues a clean system shutdown (`systemctl poweroff`).
   - Keyboard navigation supported: `Enter`, `Space`, `Y`, and `Esc` trigger "Yes, I'm awake"; `S` triggers "Shut Down Now".
4. **GUI Configuration (`omen-gui`):**
   - Added an "Inactivity & Sleep Guard" settings card under General Settings.
   - User-configurable toggle switch (Enabled / Disabled).
   - Duration selector combo box (30 minutes, 1 hour, 1.5 hours, 2 hours [default], 3 hours, 4 hours).
   - "Launch Test Prompt" button to immediately test the dialog flow and actions.
5. **CLI Administration (`omen-cli`):**
   - `omen-cli system awake-status`: View current timer status, timeout setting, idle seconds, and prompt state.
   - `omen-cli system set-awake <hours>`: Configure inactivity timeout in decimal hours (e.g. `2.0` or `0.5`).
   - `omen-cli system test-awake`: Immediately trigger the "Are you awake?" dialog over D-Bus for testing.

---

## 3. Verification & Testing Results

| Test Scenario | Executed Command | Hardware / System Response | Telemetry / Status |
| :--- | :--- | :--- | :--- |
| **Manual Fan Speed** | `omen-cli fan set-speed 70` | Target Fan Speed: 70.00% (Fan 1 & Fan 2) | Physical fan speed spins up |
| **Manual Fan Speed** | `omen-cli fan set-speed 35` | Target Fan Speed: 35.00% (Fan 1 & Fan 2) | Fans decelerate smoothly |
| **Max Fan Mode** | `omen-cli fan set-mode max` | Target Fan Speed: 100.00% | Target RPM: 6000 (Full airflow) |
| **Dynamic Auto Mode** | `omen-cli fan set-mode auto` | Curve-driven dynamic adjustment | Target RPM adjusted to thermal temp |
| **Hardware EC Auto** | `omen-cli fan set-mode ec` | Auto Control Enabled: true | Firmware takes over |
| **OMEN Key Press** | Keyboard OMEN key | D-Bus signal emitted | `omen-gui` window opens |
| **Awake Status Check** | `omen-cli system awake-status` | D-Bus returns current inactivity state | `enabled: true`, `timeout_secs: 7200`, `idle_secs: <secs>` |
| **Awake Dialog UI** | `omen-overlay --awake-dialog` | Modal countdown dialog displays on screen | 60s countdown ticks; "Yes (+1h to 3h)" and "Shut Down Now" functional |
| **Awake Test Trigger** | `omen-cli system test-awake` | D-Bus `MacroKeyPressed("awake_prompt")` emitted | `omen-tray` spawns dialog overlay |

---

## 4. Modified Files

- `src/omen-types/src/lib.rs`: Added `InactivityState` struct and D-Bus platform methods.
- `src/omen-space-daemon/src/inactivity_monitor.rs`: New hardware evdev input watcher and watchdog timer.
- `src/omen-space-daemon/src/platform.rs`: D-Bus endpoints for timer configuration and awake prompt.
- `src/omen-space-daemon/src/main.rs`: Inactivity monitor initialization on startup.
- `src/omen-tray/src/main.rs`: Intercepts `awake_prompt` signal and launches dialog overlay.
- `src/omen-overlay/src/awake_dialog.rs`: Interactive Libadwaita dialog with countdown, extensions, and keyboard shortcuts.
- `src/omen-overlay/src/daemon_client.rs`: D-Bus client calls for extending timer and powering off.
- `src/omen-overlay/src/main.rs`: Added `--awake-dialog` command-line argument.
- `src/omen-overlay/src/style.css`: Sleek glassmorphic OMEN styling for dialog and countdown.
- `src/omen-gui/src/sleep_guard.rs`: Dedicated full-page GUI for Sleep Guard with real-time idle tracking and progress bar.
- `src/omen-gui/src/performance_control.rs`: Added quick-access Sleep Guard dashboard card on the main Performance view.
- `src/omen-gui/src/main.rs`: Registered top-level Sleep Guard sidebar navigation tab.
- `src/omen-gui/src/i18n.rs`: Added translations for Sleep Guard navigation item.
- `src/omen-gui/src/settings.rs`: Added Inactivity Guard preference group and controls.
- `src/omen-gui/src/daemon_client.rs`: Synchronous & asynchronous helpers for inactivity settings.
- `src/omen-cli/src/commands/system.rs`: Added `awake-status`, `set-awake`, and `test-awake` commands.
- `src/omen-cli/src/dbus_proxy.rs`: D-Bus proxy methods for CLI system commands.
- `MODIFICATIONS.md`: Documentation of modifications and verification.
