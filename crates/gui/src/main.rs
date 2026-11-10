//! Privacy Panel: a GTK app that shows and changes the privacy settings of the apps on
//! this computer.
//!
//! Test options (not needed for normal use):
//!   --dump-json                 print everything found as JSON and exit (no window)
//!   --selftest-screenshot DIR   render every page to PNG files in DIR and exit

mod dialogs;
mod icons;
mod pkexec;
mod rows;
mod selftest;
mod ui;

use adw::prelude::*;

const APP_ID: &str = "guru.freberg.PrivacyPanel";

fn main() -> gtk::glib::ExitCode {
    // In the AppImage, GTK uses the host's GPU drivers, and older ones (Mesa 20 on
    // Ubuntu 20.04, for example) crash GTK's GL renderer. This app needs no GPU, so
    // draw in software there unless the user chose a renderer.
    if std::env::var_os("APPIMAGE").is_some() && std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "cairo");
    }
    // Built-in icons (see icons/README.md). GtkApplication adds
    // /guru/freberg/PrivacyPanel/icons to the icon theme because of the app id.
    gtk::gio::resources_register_include!("icons.gresource").expect("built-in icons");
    let args: Vec<String> = std::env::args().collect();
    let env = privacy_core::Env::from_system();

    if args.iter().any(|a| a == "--dump-json") {
        let m = privacy_core::providers::collect_all(&env);
        println!("{}", serde_json::to_string_pretty(&m).unwrap());
        return gtk::glib::ExitCode::SUCCESS;
    }
    let shots = args
        .iter()
        .position(|a| a == "--selftest-screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(if shots.is_some() { gtk::gio::ApplicationFlags::NON_UNIQUE } else { Default::default() })
        .build();
    app.connect_activate(move |app| {
        if let Some(w) = app.active_window() {
            w.present();
            return;
        }
        gtk::Window::set_default_icon_name(APP_ID);
        let ui = ui::Ui::new(app, env.clone());
        ui.window.present();
        ui.reload();
        if let Some(dir) = shots.clone() {
            selftest::run(ui, dir);
        }
    });
    // GTK must not see our own options.
    app.run_with_args(&args[..1])
}
