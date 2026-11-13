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

---

## 3. Verification & Testing Results

| Test Scenario | Executed Command | Hardware Response (`nbfc status`) | Telemetry (`omen-cli fan info`) |
| :--- | :--- | :--- | :--- |
| **Manual Fan Speed** | `omen-cli fan set-speed 70` | Target Fan Speed: 70.00% (Fan 1 & Fan 2) | Physical fan speed spins up |
| **Manual Fan Speed** | `omen-cli fan set-speed 35` | Target Fan Speed: 35.00% (Fan 1 & Fan 2) | Fans decelerate smoothly |
| **Max Fan Mode** | `omen-cli fan set-mode max` | Target Fan Speed: 100.00% | Target RPM: 6000 (Full airflow) |
| **Dynamic Auto Mode** | `omen-cli fan set-mode auto` | Curve-driven dynamic adjustment | Target RPM adjusted to thermal temp |
| **Hardware EC Auto** | `omen-cli fan set-mode ec` | Auto Control Enabled: true | Firmware takes over |
| **OMEN Key Press** | Keyboard OMEN key | D-Bus signal emitted | `omen-gui` window opens |

---

## 4. Modified Files in this Commit

- `src/omen-space-daemon/src/boards.json`: Board capabilities entry for `8A4F`.
- `src/omen-space-daemon/src/fan/mod.rs`: `nbfc` backend integration into daemon fan control pipeline.
- `src/omen-cli/src/commands/fan.rs`: Added `set-speed` subcommand.
- `MODIFICATIONS.md`: Documentation of modifications and verification.
