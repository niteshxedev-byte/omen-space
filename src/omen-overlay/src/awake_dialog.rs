use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use crate::daemon_client;

const COUNTDOWN_START_SECS: u32 = 60;
const EXTENDED_TIMER_HOURS: u64 = 3;
const EXTENDED_TIMER_SECS: u64 = EXTENDED_TIMER_HOURS * 3600;

pub fn show_awake_dialog(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Are you awake? — OMEN Space")
        .decorated(false)
        .resizable(false)
        .default_width(520)
        .default_height(340)
        .css_classes(["awake-window"])
        .build();

    let root_card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["awake-card"])
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    // ── Header ─────────────────────────────────────────────────────────────
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .valign(gtk::Align::Center)
        .build();

    let icon_img = gtk::Image::builder()
        .icon_name("system-shutdown-symbolic")
        .pixel_size(28)
        .css_classes(["awake-icon"])
        .build();
    header_box.append(&icon_img);

    let tag_label = gtk::Label::builder()
        .label("INACTIVITY GUARD")
        .css_classes(["awake-tag"])
        .halign(gtk::Align::Start)
        .build();
    header_box.append(&tag_label);

    root_card.append(&header_box);

    // ── Title & Message ────────────────────────────────────────────────────
    let title_label = gtk::Label::builder()
        .label("Are you awake?")
        .css_classes(["awake-title"])
        .halign(gtk::Align::Start)
        .build();
    root_card.append(&title_label);

    let desc_label = gtk::Label::builder()
        .label("No keyboard or trackpad activity has been detected. To prevent battery drain and excessive heat, this laptop will automatically shut down.")
        .css_classes(["awake-desc"])
        .wrap(true)
        .halign(gtk::Align::Start)
        .build();
    root_card.append(&desc_label);

    // ── Countdown Badge ────────────────────────────────────────────────────
    let countdown_secs = Rc::new(Cell::new(COUNTDOWN_START_SECS));

    let countdown_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .css_classes(["awake-countdown-box"])
        .build();

    let clock_icon = gtk::Image::builder()
        .icon_name("preferences-system-time-symbolic")
        .pixel_size(18)
        .build();
    countdown_box.append(&clock_icon);

    let countdown_label = gtk::Label::builder()
        .label(&format!("Shutting down in {} seconds...", COUNTDOWN_START_SECS))
        .css_classes(["awake-countdown-text"])
        .build();
    countdown_box.append(&countdown_label);

    root_card.append(&countdown_box);

    // ── Action Buttons ─────────────────────────────────────────────────────
    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .homogeneous(true)
        .margin_top(8)
        .build();

    let confirm_btn = gtk::Button::builder()
        .label("Yes, I'm awake (+1h to 3h)")
        .css_classes(["suggested-action", "awake-btn-confirm"])
        .build();

    let shutdown_btn = gtk::Button::builder()
        .label("Shut Down Now")
        .css_classes(["destructive-action", "awake-btn-shutdown"])
        .build();

    btn_row.append(&shutdown_btn);
    btn_row.append(&confirm_btn);
    root_card.append(&btn_row);

    window.set_content(Some(&root_card));

    // Confirm action
    let app_confirm = app.clone();
    let do_confirm = move || {
        let app_c = app_confirm.clone();
        let rt = daemon_client::get_runtime();
        rt.spawn(async move {
            let _ = daemon_client::extend_inactivity_timer(EXTENDED_TIMER_SECS).await;
            // Desktop notification
            if let Ok(conn) = zbus::Connection::session().await {
                let _ = conn.call_method(
                    Some("org.freedesktop.Notifications"),
                    "/org/freedesktop/Notifications",
                    Some("org.freedesktop.Notifications"),
                    "Notify",
                    &(
                        "OMEN Space",
                        0u32,
                        "preferences-system-time-symbolic",
                        "Inactivity Timer Extended",
                        "Timer has been increased to 3 hours. Next awake prompt in 3 hours.",
                        Vec::<&str>::new(),
                        std::collections::HashMap::<&str, zbus::zvariant::Value>::new(),
                        5000i32,
                    ),
                ).await;
            }
        });
        app_c.quit();
    };

    // Shutdown action
    let app_shutdown = app.clone();
    let do_shutdown = move || {
        let app_c = app_shutdown.clone();
        let rt = daemon_client::get_runtime();
        rt.spawn(async move {
            let _ = daemon_client::poweroff_system().await;
        });
        app_c.quit();
    };

    let do_confirm_rc = Rc::new(do_confirm);
    let do_shutdown_rc = Rc::new(do_shutdown);

    let dc1 = do_confirm_rc.clone();
    confirm_btn.connect_clicked(move |_| dc1());

    let ds1 = do_shutdown_rc.clone();
    shutdown_btn.connect_clicked(move |_| ds1());

    // Key shortcuts (Enter / Space / Y -> confirm; S -> shutdown; Esc -> confirm)
    let key_controller = gtk::EventControllerKey::new();
    let dc_key = do_confirm_rc.clone();
    let ds_key = do_shutdown_rc.clone();
    key_controller.connect_key_pressed(move |_, key, _keycode, _state| {
        let name = key.name().unwrap_or_default().to_lowercase();
        if name == "return" || name == "space" || name == "y" || name == "escape" {
            dc_key();
            glib::Propagation::Stop
        } else if name == "s" {
            ds_key();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    // Countdown timer
    let count_cell = countdown_secs.clone();
    let lbl_clone = countdown_label.clone();
    let ds_timer = do_shutdown_rc.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let remaining = count_cell.get().saturating_sub(1);
        count_cell.set(remaining);

        if remaining == 0 {
            lbl_clone.set_label("Shutting down now...");
            ds_timer();
            glib::ControlFlow::Break
        } else {
            lbl_clone.set_label(&format!("Shutting down in {} seconds...", remaining));
            glib::ControlFlow::Continue
        }
    });

    window.present();
}
