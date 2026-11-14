use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;
use std::cell::Cell;
use crate::daemon_client;

pub fn build_page(window: &adw::ApplicationWindow) -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .build();

    // ── Header Row ────────────────────────────────────────────────────────────
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .valign(gtk::Align::Center)
        .build();

    let title_vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(3)
        .hexpand(true)
        .build();

    let title_label = gtk::Label::builder()
        .label("Inactivity & Sleep Guard")
        .css_classes(["page-title"])
        .halign(gtk::Align::Start)
        .build();
    title_vbox.append(&title_label);

    let subtitle_label = gtk::Label::builder()
        .label("Monitors physical keyboard and trackpad input for unattended inactivity and prompts before shutting down.")
        .css_classes(["os-section-desc"])
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    title_vbox.append(&subtitle_label);
    header_box.append(&title_vbox);

    let status_pill = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .css_classes(["daemon-pill"])
        .valign(gtk::Align::Center)
        .build();
    let status_dot = gtk::Label::builder().label("●").build();
    let status_text = gtk::Label::builder()
        .label("GUARD ACTIVE")
        .css_classes(["daemon-pill-text"])
        .build();
    status_pill.append(&status_dot);
    status_pill.append(&status_text);
    header_box.append(&status_pill);

    page.append(&header_box);

    // ── Preferences Group: Core Guard Configuration ───────────────────────────
    let pref_page = adw::PreferencesPage::builder().build();

    let guard_group = adw::PreferencesGroup::builder()
        .title("Sleep Guard Settings")
        .description("When enabled, the daemon monitors input devices (/dev/input/event*) directly with zero compositor overhead.")
        .build();

    let init_state = daemon_client::get_inactivity_state_sync();

    // Enable Switch
    let enable_switch = adw::SwitchRow::builder()
        .title("Enable 'Are You Awake?' Auto-Shutdown")
        .subtitle("Automatically trigger awake countdown dialog when user is inactive")
        .build();
    enable_switch.set_active(init_state.enabled);
    guard_group.add(&enable_switch);

    // Timeout duration options
    let timeout_model = gtk::StringList::new(&[
        "30 Minutes",
        "1 Hour",
        "1.5 Hours",
        "2 Hours (Default)",
        "3 Hours",
        "4 Hours",
    ]);

    let selected_idx = match init_state.timeout_secs {
        1800 => 0,
        3600 => 1,
        5400 => 2,
        10800 => 4,
        14400 => 5,
        _ => 3, // Default 2 hours
    };

    let timeout_row = adw::ComboRow::builder()
        .title("Inactivity Duration Threshold")
        .subtitle("Show the 'Are you awake?' dialog after this period of no keyboard or trackpad/mouse input")
        .model(&timeout_model)
        .selected(selected_idx)
        .build();
    guard_group.add(&timeout_row);

    // Awake Behavior Row
    let behavior_row = adw::ActionRow::builder()
        .title("Awake Response Behavior")
        .subtitle("Clicking 'Yes' on the prompt extends timer by +1h to 3 Hours. Inaction triggers safe shutdown.")
        .build();
    let extend_badge = gtk::Label::builder()
        .label("+1h → 3h Total")
        .css_classes(["badge-ok"])
        .valign(gtk::Align::Center)
        .build();
    behavior_row.add_suffix(&extend_badge);
    guard_group.add(&behavior_row);

    pref_page.add(&guard_group);

    // ── Live Telemetry & Monitoring Card ───────────────────────────────────────
    let telemetry_group = adw::PreferencesGroup::builder()
        .title("Live Activity Telemetry")
        .description("Real-time hardware input tracker. Resets immediately on any physical keystroke or trackpad motion.")
        .build();

    let idle_row = adw::ActionRow::builder()
        .title("Current Inactivity Time")
        .subtitle("Elapsed time since last physical keyboard, mouse, or trackpad event")
        .build();

    let idle_value_label = gtk::Label::builder()
        .label(&format_idle_time(init_state.idle_secs))
        .css_classes(["bold", "os-section-desc"])
        .valign(gtk::Align::Center)
        .build();
    idle_row.add_suffix(&idle_value_label);
    telemetry_group.add(&idle_row);

    let progress_row = adw::ActionRow::builder()
        .title("Time Until Awake Prompt")
        .subtitle("Progress towards the configured inactivity timeout")
        .build();

    let progress_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();

    let progress_bar = gtk::ProgressBar::builder()
        .hexpand(true)
        .show_text(true)
        .text("0%")
        .build();
    progress_box.append(&progress_bar);

    let remaining_label = gtk::Label::builder()
        .label("Estimating...")
        .css_classes(["os-section-desc"])
        .build();
    progress_box.append(&remaining_label);

    progress_row.add_suffix(&progress_box);
    telemetry_group.add(&progress_row);

    pref_page.add(&telemetry_group);

    // ── Interactive Testing & Quick Actions Group ──────────────────────────────
    let actions_group = adw::PreferencesGroup::builder()
        .title("Testing & Immediate Actions")
        .description("Test the popup dialog or manually adjust the timer without waiting.")
        .build();

    let test_row = adw::ActionRow::builder()
        .title("Launch Test 'Are You Awake?' Prompt")
        .subtitle("Immediately displays the 60-second countdown dialog on your screen to verify audio/visual prompt")
        .build();

    let test_btn = gtk::Button::builder()
        .label("Launch Test Prompt")
        .css_classes(["suggested-action"])
        .valign(gtk::Align::Center)
        .build();

    test_btn.connect_clicked(|_| {
        daemon_client::trigger_awake_prompt_sync();
    });
    test_row.add_suffix(&test_btn);
    actions_group.add(&test_row);

    let extend_row = adw::ActionRow::builder()
        .title("Extend Inactivity Timer Now (+1h to 3h)")
        .subtitle("Immediately set the inactivity limit to 3 Hours (10,800s) and reset the idle timer")
        .build();

    let extend_btn = gtk::Button::builder()
        .label("Extend to 3 Hours")
        .valign(gtk::Align::Center)
        .build();

    let timeout_row_c = timeout_row.clone();
    extend_btn.connect_clicked(move |_| {
        daemon_client::extend_inactivity_timer_sync(10800);
        timeout_row_c.set_selected(4); // Select "3 Hours" in combo
    });
    extend_row.add_suffix(&extend_btn);
    actions_group.add(&extend_row);

    pref_page.add(&actions_group);

    page.append(&pref_page);

    // ── Dynamic Configuration Sync ─────────────────────────────────────────────
    let sw_clone = enable_switch.clone();
    let combo_clone = timeout_row.clone();
    let updating_cfg = Rc::new(Cell::new(false));

    let u_cfg = updating_cfg.clone();
    let sync_to_daemon = move || {
        if u_cfg.get() { return; }
        let enabled = sw_clone.is_active();
        let idx = combo_clone.selected();
        let timeout_secs = match idx {
            0 => 1800,
            1 => 3600,
            2 => 5400,
            4 => 10800,
            5 => 14400,
            _ => 7200,
        };
        daemon_client::set_inactivity_timer_sync(enabled, timeout_secs);
    };
    let sync_rc = Rc::new(sync_to_daemon);

    let s1 = sync_rc.clone();
    enable_switch.connect_active_notify(move |_| s1());

    let s2 = sync_rc.clone();
    timeout_row.connect_selected_notify(move |_| s2());

    // ── Live Telemetry Polling Loop (1-second tick) ─────────────────────────────
    let page_weak = page.downgrade();
    let idle_lbl_c = idle_value_label.clone();
    let pbar_c = progress_bar.clone();
    let rem_lbl_c = remaining_label.clone();
    let dot_c = status_dot.clone();
    let text_c = status_text.clone();

    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let Some(p) = page_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        if !p.is_mapped() {
            return glib::ControlFlow::Continue;
        }

        #[allow(deprecated)]
        let (tx, rx) = glib::MainContext::channel::<omen_types::InactivityState>(glib::Priority::default());
        let idle_lbl_2 = idle_lbl_c.clone();
        let pbar_2 = pbar_c.clone();
        let rem_lbl_2 = rem_lbl_c.clone();
        let dot_2 = dot_c.clone();
        let text_2 = text_c.clone();

        rx.attach(None, move |st| {
            if st.enabled {
                dot_2.remove_css_class("daemon-dot-off");
                dot_2.add_css_class("daemon-dot-on");
                text_2.set_label("GUARD ACTIVE");
            } else {
                dot_2.remove_css_class("daemon-dot-on");
                dot_2.add_css_class("daemon-dot-off");
                text_2.set_label("GUARD DISABLED");
            }

            idle_lbl_2.set_label(&format_idle_time(st.idle_secs));

            let fraction = if st.timeout_secs > 0 {
                (st.idle_secs as f64 / st.timeout_secs as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };
            pbar_2.set_fraction(fraction);
            pbar_2.set_text(Some(&format!("{:.0}%", fraction * 100.0)));

            let remaining_secs = st.timeout_secs.saturating_sub(st.idle_secs);
            if !st.enabled {
                rem_lbl_2.set_label("Disabled");
            } else if remaining_secs == 0 {
                rem_lbl_2.set_label("Prompt Triggered");
            } else {
                rem_lbl_2.set_label(&format!("~{}", format_duration(remaining_secs)));
            }

            glib::ControlFlow::Break
        });

        let rt = daemon_client::get_runtime();
        rt.spawn(async move {
            let st = daemon_client::get_inactivity_state_async().await;
            let _ = tx.send(st);
        });

        glib::ControlFlow::Continue
    });

    let _ = window;
    page
}

fn format_idle_time(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let s = secs % 60;
        format!("{}m {}s", mins, s)
    } else {
        let hrs = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hrs, mins)
    }
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{} min", mins)
    } else {
        let hrs = secs / 3600;
        let mins = (secs % 3600) / 60;
        if mins == 0 {
            format!("{} hr", hrs)
        } else {
            format!("{}h {}m", hrs, mins)
        }
    }
}
