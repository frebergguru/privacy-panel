//! Review recommended changes, history and undo, about.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use privacy_core::engine;
use privacy_core::{Scope, Setting};

use crate::icons;
use crate::ui::Ui;

fn dialog(title: &str, content: &gtk::Widget, bottom: Option<&gtk::Widget>) -> adw::Dialog {
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&adw::HeaderBar::new());
    tv.set_content(Some(content));
    if let Some(b) = bottom {
        tv.add_bottom_bar(b);
    }
    let d = adw::Dialog::new();
    d.set_title(title);
    d.set_child(Some(&tv));
    d.set_content_width(640);
    d.set_content_height(720);
    d
}

/// List every setting that differs from its recommendation, each with a check box.
/// `app` limits the list to one app.
pub fn review(ui: &Rc<Ui>, app: Option<String>) {
    let (items, names): (Vec<Setting>, Vec<String>) = {
        let m = ui.model.borrow();
        let items: Vec<Setting> = m
            .settings
            .iter()
            .filter(|s| s.is_editable() && s.differs_from_recommended())
            .filter(|s| app.as_ref().is_none_or(|a| &s.app_id == a))
            .cloned()
            .collect();
        let names = items
            .iter()
            .map(|s| m.app(&s.app_id).map(|a| a.name.clone()).unwrap_or_default())
            .collect();
        (items, names)
    };
    if items.is_empty() {
        ui.toast("Nothing to change: everything that can be changed already matches the recommendations.");
        return;
    }

    let page = adw::PreferencesPage::new();
    let checks: Rc<RefCell<Vec<(String, gtk::CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));
    let mut cur_group: Option<(String, adw::PreferencesGroup)> = None;
    let intro = adw::PreferencesGroup::new();
    intro.set_description(Some(
        "Untick anything you want to keep as it is. Changes marked \"needs the administrator password\" are made together after one password prompt. Everything can be undone later.",
    ));
    page.add(&intro);
    for (s, app_name) in items.iter().zip(&names) {
        let key = format!("{app_name}\u{1f}{}", s.group.clone().unwrap_or_default());
        if cur_group.as_ref().is_none_or(|(k, _)| *k != key) {
            let g = adw::PreferencesGroup::new();
            g.set_title(&glib::markup_escape_text(app_name));
            if let Some(gr) = &s.group {
                g.set_description(Some(&glib::markup_escape_text(gr)));
            }
            page.add(&g);
            cur_group = Some((key, g));
        }
        let row = adw::ActionRow::new();
        row.set_use_markup(false);
        row.set_title(&s.title);
        let now = s.current_label();
        let after = s.recommended.as_ref().map(|v| s.value_label(v)).unwrap_or_default();
        let mut sub = format!("Now: {now}. After: {after}.");
        if s.scope() == Scope::Admin {
            sub.push_str(" Needs the administrator password.");
        }
        row.set_subtitle(&sub);
        let cb = gtk::CheckButton::new();
        cb.set_active(true);
        cb.set_valign(gtk::Align::Center);
        cb.update_property(&[
            gtk::accessible::Property::Label(&format!("{}: {}", app_name, s.title)),
            gtk::accessible::Property::Description(&sub),
        ]);
        row.add_prefix(&cb);
        row.add_suffix(&crate::rows::info_button(ui, s));
        row.set_activatable_widget(Some(&cb));
        cur_group.as_ref().unwrap().1.add(&row);
        checks.borrow_mut().push((s.id.clone(), cb));
    }

    let apply = gtk::Button::with_label(&format!("Apply {} changes", items.len()));
    apply.add_css_class("suggested-action");
    apply.add_css_class("pill");
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_top(12);
    bar.set_margin_bottom(12);
    bar.append(&apply);

    let d = dialog("Recommended changes", &page.clone().upcast(), Some(&bar.upcast()));

    for (_, cb) in checks.borrow().iter() {
        let checks = checks.clone();
        let apply = apply.clone();
        cb.connect_toggled(move |_| {
            let n = checks.borrow().iter().filter(|(_, c)| c.is_active()).count();
            apply.set_label(&format!("Apply {n} changes"));
            apply.set_sensitive(n > 0);
        });
    }

    let ui2 = ui.clone();
    let dw = d.downgrade();
    apply.connect_clicked(move |btn| {
        let chosen: Vec<String> = checks.borrow().iter().filter(|(_, c)| c.is_active()).map(|(id, _)| id.clone()).collect();
        let changes = {
            let m = ui2.model.borrow();
            engine::plan_recommended(&m, m.settings.iter().filter(|s| chosen.contains(&s.id)))
        };
        btn.set_sensitive(false);
        btn.set_label("Applying...");
        let dw = dw.clone();
        let ui3 = ui2.clone();
        ui2.apply(
            changes,
            Some(Box::new(move || {
                if let Some(d) = dw.upgrade() {
                    d.close();
                }
                let p = ui3.current_page();
                ui3.show(p);
            })),
        );
    });
    d.present(Some(&ui.window));
}

fn fmt_time(secs: u64) -> String {
    glib::DateTime::from_unix_local(secs as i64)
        .and_then(|t| t.format("%Y-%m-%d %H:%M"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| secs.to_string())
}

pub fn history(ui: &Rc<Ui>) {
    let batches = privacy_core::history::batches(&ui.env);
    let undone = privacy_core::history::undone_ids(&ui.env);
    let page = adw::PreferencesPage::new();
    let d_cell: Rc<RefCell<Option<adw::Dialog>>> = Rc::new(RefCell::new(None));

    if batches.is_empty() {
        let g = adw::PreferencesGroup::new();
        g.set_description(Some("No changes have been made with Privacy Panel yet."));
        page.add(&g);
    } else {
        let g = adw::PreferencesGroup::new();
        g.set_title("Changes, newest first");
        g.set_description(Some(&format!(
            "Backups of every file changed are kept in {}.",
            glib::markup_escape_text(&ui.env.app_state().join("backups").display().to_string())
        )));
        for b in batches.iter().take(200) {
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            let what = match b.undo_of {
                Some(0) => "Restore everything".to_string(),
                Some(_) => "Undo".to_string(),
                None => format!("{} change{}", b.entries.len(), if b.entries.len() == 1 { "" } else { "s" }),
            };
            row.set_title(&format!("{}: {what}", fmt_time(b.time)));
            let titles: Vec<String> = b
                .entries
                .iter()
                .map(|e| if e.app_name.is_empty() { e.title.clone() } else { format!("{} ({})", e.title, e.app_name) })
                .collect();
            let mut sub = titles.iter().take(4).cloned().collect::<Vec<_>>().join("; ");
            if titles.len() > 4 {
                sub.push_str(&format!("; and {} more", titles.len() - 4));
            }
            row.set_subtitle(&sub);
            if b.undo_of.is_none() {
                if undone.contains(&b.id) {
                    let l = gtk::Label::new(Some("Undone"));
                    l.add_css_class("dim-label");
                    row.add_suffix(&l);
                } else {
                    let btn = gtk::Button::with_label("Undo");
                    btn.set_valign(gtk::Align::Center);
                    btn.update_property(&[gtk::accessible::Property::Label(&format!("Undo the changes from {}", fmt_time(b.time)))]);
                    let ui2 = ui.clone();
                    let id = b.id;
                    let dc = d_cell.clone();
                    btn.connect_clicked(move |_| {
                        if let Some(d) = dc.borrow().as_ref() {
                            d.close();
                        }
                        ui2.undo(id);
                    });
                    row.add_suffix(&btn);
                }
            }
            g.add(&row);
        }
        page.add(&g);

        let g = adw::PreferencesGroup::new();
        let row = adw::ActionRow::new();
        row.set_title("Restore everything");
        row.set_subtitle("Put every setting Privacy Panel has changed back to how it was before its first change.");
        let btn = gtk::Button::with_label("Restore");
        btn.add_css_class("destructive-action");
        btn.set_valign(gtk::Align::Center);
        btn.update_property(&[gtk::accessible::Property::Label("Restore everything")]);
        row.add_suffix(&btn);
        row.set_activatable_widget(Some(&btn));
        let ui2 = ui.clone();
        let dc = d_cell.clone();
        btn.connect_clicked(move |_| {
            let a = adw::AlertDialog::new(
                Some("Restore everything?"),
                Some("Every setting changed with Privacy Panel goes back to how it was before. System settings ask for the administrator password."),
            );
            a.add_response("cancel", "Cancel");
            a.add_response("restore", "Restore");
            a.set_response_appearance("restore", adw::ResponseAppearance::Destructive);
            a.set_default_response(Some("cancel"));
            a.set_close_response("cancel");
            let ui3 = ui2.clone();
            let dc2 = dc.clone();
            a.connect_response(None, move |_, r| {
                if r == "restore" {
                    if let Some(d) = dc2.borrow().as_ref() {
                        d.close();
                    }
                    ui3.restore_everything();
                }
            });
            a.present(Some(&ui2.window));
        });
        g.add(&row);
        page.add(&g);
    }
    let d = dialog("History and undo", &page.upcast(), None);
    *d_cell.borrow_mut() = Some(d.clone());
    d.present(Some(&ui.window));
}

pub fn about(ui: &Rc<Ui>) {
    let a = adw::AboutDialog::builder()
        .application_name("Privacy Panel")
        .application_icon(icons::resolve(icons::APP))
        .version(env!("CARGO_PKG_VERSION"))
        .license_type(gtk::License::Gpl30)
        .comments("Shows the privacy settings of the apps on this computer and lets you change them, one by one or all at once. Every change can be undone.\n\nPrivacy Panel only reads settings on this computer. It sends nothing anywhere.")
        .build();
    a.present(Some(&ui.window));
}
