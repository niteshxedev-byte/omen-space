use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use evdev::{Device, Key};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::io::unix::AsyncFd;
use tokio::sync::RwLock;
use zbus::Connection;
use omen_types::InactivityState;

const CONFIG_PATH: &str = "/etc/omen-space/inactivity.json";
const DEFAULT_TIMEOUT_SECS: u64 = 7200; // 2 hours
const EXTENDED_TIMEOUT_SECS: u64 = 10800; // 3 hours
const PROMPT_GRACE_SECS: u64 = 75; // Grace period before automatic shutdown

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InactivityConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl Default for InactivityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }
}

impl InactivityConfig {
    pub fn load() -> Self {
        if let Ok(data) = std::fs::read_to_string(CONFIG_PATH) {
            if let Ok(cfg) = serde_json::from_str::<InactivityConfig>(&data) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all("/etc/omen-space");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(CONFIG_PATH, json);
        }
    }
}

static LAST_INPUT_SECS: AtomicU64 = AtomicU64::new(0);
static PROMPT_ACTIVE: AtomicBool = AtomicBool::new(false);
static PROMPT_START_SECS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_CONFIG: std::sync::OnceLock<Arc<RwLock<InactivityConfig>>> = std::sync::OnceLock::new();
static DBUS_CONN: std::sync::OnceLock<Connection> = std::sync::OnceLock::new();

fn get_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub struct InactivityMonitor;

impl InactivityMonitor {
    pub fn start(conn: Connection) {
        let now = get_now_secs();
        LAST_INPUT_SECS.store(now, Ordering::SeqCst);
        let _ = DBUS_CONN.set(conn.clone());

        let cfg = Arc::new(RwLock::new(InactivityConfig::load()));
        let _ = GLOBAL_CONFIG.set(cfg.clone());

        let conn_clone = conn.clone();
        tokio::spawn(async move {
            Self::event_listener_loop().await;
        });

        tokio::spawn(async move {
            Self::watchdog_loop(conn_clone, cfg).await;
        });
    }

    pub async fn get_state() -> InactivityState {
        let now = get_now_secs();
        let last = LAST_INPUT_SECS.load(Ordering::Relaxed);
        let idle = now.saturating_sub(last);
        let prompt = PROMPT_ACTIVE.load(Ordering::Relaxed);

        if let Some(cfg_lock) = GLOBAL_CONFIG.get() {
            let cfg = cfg_lock.read().await;
            InactivityState {
                enabled: cfg.enabled,
                timeout_secs: cfg.timeout_secs,
                idle_secs: idle,
                prompt_active: prompt,
            }
        } else {
            InactivityState {
                enabled: false,
                timeout_secs: DEFAULT_TIMEOUT_SECS,
                idle_secs: idle,
                prompt_active: prompt,
            }
        }
    }

    pub async fn set_timer(enabled: bool, timeout_secs: u64) -> String {
        let now = get_now_secs();
        LAST_INPUT_SECS.store(now, Ordering::SeqCst);
        PROMPT_ACTIVE.store(false, Ordering::SeqCst);

        let timeout = if timeout_secs == 0 { DEFAULT_TIMEOUT_SECS } else { timeout_secs };

        if let Some(cfg_lock) = GLOBAL_CONFIG.get() {
            let mut cfg = cfg_lock.write().await;
            cfg.enabled = enabled;
            cfg.timeout_secs = timeout;
            cfg.save();
            info!("Inactivity timer updated: enabled={}, timeout={}s", enabled, timeout);
        }
        "OK".to_string()
    }

    pub async fn extend_timer(new_timeout_secs: u64) -> String {
        let now = get_now_secs();
        LAST_INPUT_SECS.store(now, Ordering::SeqCst);
        PROMPT_ACTIVE.store(false, Ordering::SeqCst);

        let timeout = if new_timeout_secs == 0 { EXTENDED_TIMEOUT_SECS } else { new_timeout_secs };

        if let Some(cfg_lock) = GLOBAL_CONFIG.get() {
            let mut cfg = cfg_lock.write().await;
            cfg.enabled = true;
            cfg.timeout_secs = timeout;
            cfg.save();
            info!("Inactivity timer extended: timeout={}s", timeout);
        }
        "OK".to_string()
    }

    pub async fn trigger_awake_prompt() -> String {
        let now = get_now_secs();
        PROMPT_ACTIVE.store(true, Ordering::SeqCst);
        PROMPT_START_SECS.store(now, Ordering::SeqCst);

        if let Some(conn) = DBUS_CONN.get() {
            info!("Manually triggering awake_prompt signal...");
            let _ = conn.emit_signal(
                None::<&str>,
                "/org/hp/omen/Platform",
                "org.hp.omen.Platform",
                "MacroKeyPressed",
                &("awake_prompt"),
            ).await;
        }
        "OK".to_string()
    }

    pub fn poweroff_system() -> String {
        info!("Executing system poweroff via systemctl poweroff...");
        tokio::spawn(async move {
            let _ = tokio::process::Command::new("/usr/bin/systemctl")
                .arg("poweroff")
                .spawn();
        });
        "OK".to_string()
    }

    async fn watchdog_loop(conn: Connection, cfg_lock: Arc<RwLock<InactivityConfig>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;

            let (enabled, timeout_secs) = {
                let cfg = cfg_lock.read().await;
                (cfg.enabled, cfg.timeout_secs)
            };

            if !enabled {
                continue;
            }

            let now = get_now_secs();
            let last_input = LAST_INPUT_SECS.load(Ordering::Relaxed);
            let idle_secs = now.saturating_sub(last_input);
            let prompt_active = PROMPT_ACTIVE.load(Ordering::Relaxed);

            if !prompt_active {
                if idle_secs >= timeout_secs {
                    info!(
                        "Inactivity timeout reached: {}s idle >= {}s limit. Triggering 'Are you awake?' dialog.",
                        idle_secs, timeout_secs
                    );
                    PROMPT_ACTIVE.store(true, Ordering::SeqCst);
                    PROMPT_START_SECS.store(now, Ordering::SeqCst);

                    let _ = conn.emit_signal(
                        None::<&str>,
                        "/org/hp/omen/Platform",
                        "org.hp.omen.Platform",
                        "MacroKeyPressed",
                        &("awake_prompt"),
                    ).await;
                }
            } else {
                let prompt_start = PROMPT_START_SECS.load(Ordering::Relaxed);
                let elapsed_since_prompt = now.saturating_sub(prompt_start);

                if elapsed_since_prompt >= PROMPT_GRACE_SECS {
                    warn!(
                        "Awake dialog was not answered within {}s grace period. Shutting down system now.",
                        elapsed_since_prompt
                    );
                    PROMPT_ACTIVE.store(false, Ordering::SeqCst);
                    let _ = tokio::process::Command::new("/usr/bin/systemctl")
                        .arg("poweroff")
                        .spawn();
                    break;
                }
            }
        }
    }

    async fn event_listener_loop() {
        loop {
            let mut matching_devices: Vec<std::path::PathBuf> = Vec::new();

            if let Ok(mut dir) = tokio::fs::read_dir("/dev/input").await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let path = entry.path();
                    if path.to_string_lossy().contains("event") {
                        if let Ok(dev) = Device::open(&path) {
                            let is_keyboard = dev.supported_keys().is_some_and(|k| {
                                k.contains(Key::KEY_A)
                                    || k.contains(Key::KEY_SPACE)
                                    || k.contains(Key::KEY_ENTER)
                            });
                            let is_mouse = dev.supported_relative_axes().is_some_and(|a| {
                                a.contains(evdev::RelativeAxisType::REL_X)
                                    || a.contains(evdev::RelativeAxisType::REL_Y)
                            }) || dev.supported_keys().is_some_and(|k| {
                                k.contains(Key::BTN_LEFT) || k.contains(Key::BTN_RIGHT)
                            });
                            let is_touchpad = dev.supported_absolute_axes().is_some_and(|a| {
                                a.contains(evdev::AbsoluteAxisType::ABS_X)
                                    || a.contains(evdev::AbsoluteAxisType::ABS_MT_POSITION_X)
                            }) || dev.supported_keys().is_some_and(|k| {
                                k.contains(Key::BTN_TOUCH) || k.contains(Key::BTN_TOOL_FINGER)
                            });

                            if is_keyboard || is_mouse || is_touchpad {
                                matching_devices.push(path);
                            }
                        }
                    }
                }
            }

            if matching_devices.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            let mut join_set = tokio::task::JoinSet::new();
            for path in matching_devices {
                join_set.spawn(async move {
                    let Ok(file) = std::fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(&path) else { return; };
                    let Ok(async_fd) = AsyncFd::new(file) else { return; };

                    let event_size = std::mem::size_of::<libc::input_event>();
                    let mut read_buf = vec![0u8; event_size * 32];

                    loop {
                        let Ok(mut guard) = async_fd.readable().await else { break; };
                        let try_res = guard.try_io(|_| {
                            let fd = async_fd.get_ref().as_raw_fd();
                            let n = unsafe {
                                libc::read(fd, read_buf.as_mut_ptr() as *mut libc::c_void, read_buf.len())
                            };
                            if n < 0 {
                                Err(std::io::Error::last_os_error())
                            } else {
                                Ok(n as usize)
                            }
                        });

                        match try_res {
                            Ok(Ok(bytes_read)) if bytes_read >= event_size => {
                                let count = bytes_read / event_size;
                                let events = unsafe {
                                    std::slice::from_raw_parts(read_buf.as_ptr() as *const libc::input_event, count)
                                };
                                for ev in events {
                                    // Filter EV_SYN (0)
                                    if ev.type_ == 1 || ev.type_ == 2 || ev.type_ == 3 {
                                        // Update activity timestamp
                                        let now = get_now_secs();
                                        LAST_INPUT_SECS.store(now, Ordering::Relaxed);
                                    }
                                }
                            }
                            Ok(Ok(_)) => {}
                            Ok(Err(e)) => {
                                if e.raw_os_error() == Some(libc::ENODEV) || e.raw_os_error() == Some(libc::EBADF) {
                                    break;
                                }
                            }
                            Err(_would_block) => {}
                        }
                    }
                });
            }

            let _ = join_set.join_next().await;
            join_set.abort_all();
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    }
}
