//! One row per setting: a switch, a drop-down, an information line or an action button.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::accessible::Property;

use privacy_core::{Current, Kind, Scope, Setting, Value};

use crate::icons;
use crate::ui::Ui;

/// Short status in words, so nothing depends on colour alone.
pub fn status_text(s: &Setting) -> (String, &'static [&'static str]) {
    if let Some(app) = &s.blocked_by {
        return (format!("Close {app} first"), icons::INFO);
    }
    match &s.current {
        Current::Unavailable(why) => (why.clone(), icons::QUESTION),
        Current::Info(_) => (String::new(), &[]),
        _ if s.differs_from_recommended() => {
            let r = s.recommended.as_ref().map(|v| s.value_label(v)).unwrap_or_default();
            (format!("Recommended: {r}"), icons::WARNING)
        }
        Current::Value { .. } if s.recommended.is_some() => ("Recommended".into(), icons::OK),
        Current::Value { is_default: true, .. } => ("Default".into(), &[]),
        _ => (String::new(), &[]),
    }
}

fn subtitle(s: &Setting, app_name: Option<&str>) -> String {
    let mut t = String::new();
    if let Some(a) = app_name {
        t.push_str(a);
        if let Some(g) = &s.group {
            t.push_str(", ");
            t.push_str(g);
        }
        t.push_str(". ");
    }
    t.push_str(&s.explanation);
    if s.scope() == Scope::Admin {
        t.push_str(" Needs the administrator password.");
    }
    if let Current::Other(raw) = &s.current {
        t.push_str(&format!(" Currently set to a custom value: {raw}."));
    }
    t
}

struct Status {
    icon: gtk::Image,
    label: gtk::Label,
}

fn status_widgets() -> (gtk::Box, Status) {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    b.set_valign(gtk::Align::Center);
    let icon = gtk::Image::new();
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    let label = gtk::Label::new(None);
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_width_chars(10);
    label.set_max_width_chars(26);
    label.set_xalign(1.0);
    b.append(&icon);
    b.append(&label);
    (b, Status { icon, label })
}

fn update_status(st: &Status, s: &Setting) {
    let (text, icon) = status_text(s);
    st.label.set_text(&text);
    st.label.set_visible(!text.is_empty());
    let name = (!icon.is_empty()).then(|| icons::resolve(icon));
    st.icon.set_icon_name(name.as_deref());
    st.icon.set_visible(!icon.is_empty());
    st.label.remove_css_class("warning");
    st.icon.remove_css_class("warning");
    if s.differs_from_recommended() && s.blocked_by.is_none() {
        st.label.add_css_class("warning");
        st.icon.add_css_class("warning");
    }
}

fn prefix_icon(row: &adw::ActionRow, s: &Setting) {
    if s.scope() == Scope::Admin {
        let lock = gtk::Image::from_icon_name("changes-prevent-symbolic");
        lock.set_tooltip_text(Some("Needs the administrator password"));
        lock.set_accessible_role(gtk::AccessibleRole::Presentation);
        row.add_prefix(&lock);
    }
}

fn section_label(b: &gtk::Box, heading: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let h = gtk::Label::new(Some(heading));
    h.add_css_class("heading");
    h.set_xalign(0.0);
    h.set_accessible_role(gtk::AccessibleRole::Heading);
    h.set_margin_top(6);
    let t = gtk::Label::new(Some(text));
    t.set_wrap(true);
    t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    t.set_xalign(0.0);
    t.set_max_width_chars(52);
    t.set_selectable(true);
    b.append(&h);
    b.append(&t);
}

/// The "What is this?" button: what the thing is, what the setting does, and exactly
/// what changes on the computer. The popover is built when it is opened.
pub fn info_button(ui: &Rc<Ui>, s: &Setting) -> gtk::MenuButton {
    let btn = gtk::MenuButton::new();
    btn.set_icon_name("help-about-symbolic");
    btn.add_css_class("flat");
    btn.add_css_class("circular");
    btn.set_valign(gtk::Align::Center);
    btn.set_tooltip_text(Some("What is this?"));
    btn.update_property(&[Property::Label(&format!("What is this: {}", s.title))]);
    let s = s.clone();
    let env = ui.env.clone();
    btn.set_create_popup_func(move |b| {
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 4);
        bx.set_margin_top(12);
        bx.set_margin_bottom(12);
        bx.set_margin_start(12);
        bx.set_margin_end(12);
        let title = gtk::Label::new(Some(&s.title));
        title.add_css_class("title-4");
        title.set_wrap(true);
        title.set_xalign(0.0);
        title.set_max_width_chars(52);
        bx.append(&title);
        if let Some(a) = &s.about {
            section_label(&bx, "What is this?", a);
        }
        section_label(&bx, "What this setting does", &s.explanation);
        let mut now = format!("Now: {}.", s.current_label());
        if let Some(r) = &s.recommended {
            now.push_str(&format!(" Privacy Panel recommends: {}.", s.value_label(r)));
        }
        if let Some(app) = &s.blocked_by {
            now.push_str(&format!(" Close {app} before changing it, because {app} overwrites its settings while it runs."));
        }
        section_label(&bx, "Current state", &now);
        if let Some(c) = privacy_core::describe::change(&env, &s) {
            section_label(&bx, "What changes on your computer", &c);
            section_label(&bx, "Undo", "Every change is recorded. Use History and undo in the main menu to put it back.");
        }
        let sc = gtk::ScrolledWindow::new();
        sc.set_hscrollbar_policy(gtk::PolicyType::Never);
        sc.set_propagate_natural_height(true);
        sc.set_max_content_height(460);
        sc.set_child(Some(&bx));
        let pop = gtk::Popover::new();
        pop.set_child(Some(&sc));
        b.set_popover(Some(&pop));
    });
    btn
}

/// Build the row for one setting. `app_name` is shown when the row appears outside its app.
pub fn build(ui: &Rc<Ui>, s: &Setting, app_name: Option<&str>) -> gtk::Widget {
    match &s.kind {
        Kind::Bool => bool_row(ui, s, app_name),
        Kind::Choice(_) => choice_row(ui, s, app_name),
        Kind::Info => info_row(ui, s, app_name),
        Kind::Action(kind) => action_row(ui, s, *kind, app_name),
    }
}

fn bool_row(ui: &Rc<Ui>, s: &Setting, app_name: Option<&str>) -> gtk::Widget {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&s.title);
    row.set_subtitle(&subtitle(s, app_name));
    prefix_icon(&row, s);
    let (sbox, status) = status_widgets();
    row.add_suffix(&sbox);
    row.add_suffix(&info_button(ui, s));
    let sw = gtk::Switch::new();
    sw.set_valign(gtk::Align::Center);
    row.add_suffix(&sw);
    row.set_activatable_widget(Some(&sw));

    let guard = Rc::new(Cell::new(false));
    let status = Rc::new(status);
    let update = {
        let sw = sw.downgrade();
        let row = row.downgrade();
        let guard = guard.clone();
        let status = status.clone();
        move |s: &Setting| {
            let (Some(sw), Some(row)) = (sw.upgrade(), row.upgrade()) else { return };
            guard.set(true);
            let on = s.current.value().and_then(Value::as_bool).unwrap_or(false);
            sw.set_active(on);
            sw.set_sensitive(s.is_editable());
            guard.set(false);
            let app = row_app_name(&row);
            row.set_subtitle(&subtitle(s, app.as_deref()));
            update_status(&status, s);
            let (st, _) = status_text(s);
            sw.update_property(&[
                Property::Label(&s.title),
                Property::Description(&format!("{}. {}", s.current_label(), st)),
            ]);
        }
    };
    // Remember the app name for later subtitle updates.
    if let Some(a) = app_name {
        unsafe { row.set_data("pp-app-name", a.to_string()) };
    }
    update(s);
    let id = s.id.clone();
    let ui2 = ui.clone();
    sw.connect_active_notify(move |sw| {
        if guard.get() {
            return;
        }
        ui2.change(&id, Value::Bool(sw.is_active()));
    });
    ui.register(&s.id, Box::new(update));
    row.upcast()
}

fn row_app_name(row: &adw::ActionRow) -> Option<String> {
    unsafe { row.data::<String>("pp-app-name").map(|p| p.as_ref().clone()) }
}

fn choice_row(ui: &Rc<Ui>, s: &Setting, app_name: Option<&str>) -> gtk::Widget {
    let Kind::Choice(opts) = &s.kind else { unreachable!() };
    // A plain row with a separate drop-down: AdwComboRow cuts the chosen value short
    // when the explanation is long.
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&s.title);
    row.set_subtitle(&subtitle(s, app_name));
    if let Some(a) = app_name {
        unsafe { row.set_data("pp-app-name", a.to_string()) };
    }
    prefix_icon(&row, s);
    let (sbox, status) = status_widgets();
    row.add_suffix(&sbox);
    row.add_suffix(&info_button(ui, s));
    let labels: Vec<String> = opts.iter().map(|o| o.label.clone()).collect();
    let model = gtk::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    let dd = gtk::DropDown::new(Some(model.clone()), None::<gtk::Expression>);
    dd.set_valign(gtk::Align::Center);
    dd.update_property(&[Property::Label(&s.title)]);
    row.add_suffix(&dd);
    row.set_activatable_widget(Some(&dd));

    let guard = Rc::new(Cell::new(false));
    let status = Rc::new(status);
    let ids: Rc<Vec<String>> = Rc::new(opts.iter().map(|o| o.id.clone()).collect());
    let n_real = opts.len() as u32;
    let update = {
        let row_w = row.downgrade();
        let dd_w = dd.downgrade();
        let guard = guard.clone();
        let ids = ids.clone();
        let model = model.clone();
        let status = status.clone();
        move |s: &Setting| {
            let (Some(row), Some(dd)) = (row_w.upgrade(), dd_w.upgrade()) else { return };
            guard.set(true);
            // Drop a previous "custom value" entry.
            while model.n_items() > n_real {
                model.remove(model.n_items() - 1);
            }
            let idx = match &s.current {
                Current::Value { value: Value::Choice(id), .. } => ids.iter().position(|x| x == id).map(|i| i as u32),
                Current::Other(raw) => {
                    model.append(&format!("Custom value: {raw}"));
                    Some(n_real)
                }
                _ => None,
            };
            dd.set_selected(idx.unwrap_or(gtk::INVALID_LIST_POSITION));
            dd.set_sensitive(s.is_editable());
            let app = row_app_name(&row);
            let (st, _) = status_text(s);
            // Status in words goes in the subtitle; only its icon sits by the drop-down.
            let mut sub = subtitle(s, app.as_deref());
            if !st.is_empty() {
                sub.push_str(&format!(" {st}."));
            }
            row.set_subtitle(&sub);
            update_status(&status, s);
            status.label.set_visible(false);
            dd.update_property(&[Property::Description(&format!("{}. {}", s.current_label(), st))]);
            guard.set(false);
        }
    };
    update(s);
    let id = s.id.clone();
    let ui2 = ui.clone();
    dd.connect_selected_notify(move |dd| {
        if guard.get() {
            return;
        }
        let i = dd.selected() as usize;
        if let Some(choice) = ids.get(i) {
            ui2.change(&id, Value::Choice(choice.clone()));
        }
    });
    ui.register(&s.id, Box::new(update));
    row.upcast()
}

fn info_row(ui: &Rc<Ui>, s: &Setting, app_name: Option<&str>) -> gtk::Widget {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&s.title);
    row.set_subtitle(&subtitle(s, app_name));
    let text = match &s.current {
        Current::Info(t) => t.clone(),
        other => format!("{other:?}"),
    };
    let label = gtk::Label::new(Some(&text));
    label.set_wrap(true);
    label.set_max_width_chars(30);
    label.set_xalign(1.0);
    label.set_selectable(true);
    label.add_css_class("dim-label");
    row.add_suffix(&label);
    if s.about.is_some() {
        row.add_suffix(&info_button(ui, s));
    }
    row.upcast()
}

fn action_row(ui: &Rc<Ui>, s: &Setting, kind: privacy_core::ActionKind, app_name: Option<&str>) -> gtk::Widget {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&s.title);
    row.set_subtitle(&subtitle(s, app_name));
    let label = match &s.current {
        Current::Info(t) if !t.is_empty() => t.clone(),
        _ => "Run".into(),
    };
    let btn = gtk::Button::with_label(&label);
    btn.set_valign(gtk::Align::Center);
    btn.add_css_class("destructive-action");
    btn.update_property(&[Property::Label(&format!("{label}: {}", s.title))]);
    if s.about.is_some() {
        row.add_suffix(&info_button(ui, s));
    }
    row.add_suffix(&btn);
    row.set_activatable_widget(Some(&btn));
    let ui2 = ui.clone();
    let title = s.title.clone();
    let expl = s.explanation.clone();
    btn.connect_clicked(move |_| ui2.confirm_action(kind, &title, &expl, &label));
    row.upcast()
}
