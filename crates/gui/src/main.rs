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
