//! Render each page to a PNG for checking the layout without a visible window
//! (run with GDK_BACKEND=broadway or under a headless compositor).

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{glib, gsk};

use privacy_core::Section;

use crate::ui::{Page, Ui};

fn save(ui: &Ui, path: &std::path::Path) -> bool {
    let w = ui.window.width();
    let h = ui.window.height();
    let paintable = gtk::WidgetPaintable::new(Some(&ui.window));
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, w as f64, h as f64);
    let Some(node) = snap.to_node() else {
        return false;
    };
    let renderer = gsk::CairoRenderer::new();
    if renderer.realize(None::<&gtk::gdk::Surface>).is_err() {
        eprintln!("selftest: could not start the renderer");
        return true;
    }
    let tex = renderer.render_texture(&node, None);
    renderer.unrealize();
    match tex.save_to_png(path) {
        Ok(()) => println!("selftest: wrote {}", path.display()),
        Err(e) => eprintln!("selftest: {}: {e}", path.display()),
    }
    true
}

pub fn run(ui: Rc<Ui>, dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    let mut steps: Vec<(String, Box<dyn Fn(&Rc<Ui>)>)> = vec![
        ("01-overview".into(), Box::new(|u| u.show(Page::Overview))),
        ("02-system".into(), Box::new(|u| u.show(Page::Section(Section::System)))),
        ("03-network".into(), Box::new(|u| u.show(Page::Section(Section::Network)))),
        ("04-browsers".into(), Box::new(|u| u.show(Page::Section(Section::Browsers)))),
        ("05-applications".into(), Box::new(|u| u.show(Page::Section(Section::Applications)))),
        ("06-permissions".into(), Box::new(|u| u.show(Page::Section(Section::Permissions)))),
        ("07-sandbox".into(), Box::new(|u| u.show(Page::Section(Section::Sandbox)))),
        ("08-all-apps".into(), Box::new(|u| u.show(Page::AllApps))),
        ("09-search".into(), Box::new(|u| u.show(Page::Search("telemetry".into())))),
        ("10-app-firefox".into(), Box::new(|u| { u.show(Page::AllApps); u.push_app("firefox"); })),
        ("11-review".into(), Box::new(|u| crate::dialogs::review(u, None))),
    ];
    steps.reverse();
    let steps = Rc::new(std::cell::RefCell::new(steps));
    // Wait until the model is loaded, then step through the pages.
    let ui2 = ui.clone();
    glib::timeout_add_local(Duration::from_millis(300), move || {
        if ui2.model.borrow().settings.is_empty() {
            return glib::ControlFlow::Continue;
        }
        let ui3 = ui2.clone();
        let steps = steps.clone();
        let dir = dir.clone();
        let pending: Rc<std::cell::RefCell<Option<String>>> = Rc::new(std::cell::RefCell::new(None));
        let tries = Rc::new(std::cell::Cell::new(0u32));
        glib::timeout_add_local(Duration::from_millis(700), move || {
            let p = pending.borrow().clone();
            if let Some(name) = p {
                // Not drawn yet: try again on the next tick, a few times.
                if !save(&ui3, &dir.join(format!("{name}.png"))) {
                    let n = tries.get() + 1;
                    tries.set(n);
                    if n < 10 {
                        return glib::ControlFlow::Continue;
                    }
                    eprintln!("selftest: gave up on {name}");
                }
                tries.set(0);
                pending.borrow_mut().take();
            }
            match steps.borrow_mut().pop() {
                Some((name, f)) => {
                    f(&ui3);
                    *pending.borrow_mut() = Some(name);
                    glib::ControlFlow::Continue
                }
                None => {
                    if let Some(app) = ui3.window.application() {
                        app.quit();
                    }
                    glib::ControlFlow::Break
                }
            }
        });
        glib::ControlFlow::Break
    });
}
