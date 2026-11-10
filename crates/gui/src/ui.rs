//! The main window: sidebar, pages, applying changes.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use privacy_core::engine::{self, PlannedChange, Report};
use privacy_core::{ActionKind, App, AppSource, Category, Env, Model, Section, Setting, Value};

use crate::icons;
use crate::pkexec::PkexecHelper;
use crate::rows;

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Overview,
    Section(Section),
    AllApps,
    Category(Category),
    Search(String),
}

type Updater = Box<dyn Fn(&Setting)>;

pub struct Ui {
    pub env: Env,
    pub model: RefCell<Model>,
    pub window: adw::ApplicationWindow,
    split: adw::NavigationSplitView,
    nav: adw::NavigationView,
    sidebar: gtk::ListBox,
    search: gtk::SearchEntry,
    toasts: adw::ToastOverlay,
    updaters: RefCell<HashMap<String, Vec<Updater>>>,
    page: RefCell<Page>,
    sidebar_pages: Vec<(Page, gtk::Label)>,
    busy: Cell<bool>,
    spinner: gtk::Spinner,
}

const SIDEBAR: &[(&str, &[&str])] = &[
    ("Overview", &["view-grid-symbolic"]),
    ("System", &["computer-symbolic"]),
    ("Network", &["network-wireless-symbolic"]),
    ("Browsers", icons::BROWSERS),
    ("Applications", icons::APPLICATIONS),
    ("App permissions", &["security-high-symbolic"]),
    ("Sandboxed apps", icons::SANDBOX),
    ("All installed apps", icons::LIST),
];

fn sidebar_page(i: usize) -> Page {
    match i {
        0 => Page::Overview,
        1 => Page::Section(Section::System),
        2 => Page::Section(Section::Network),
        3 => Page::Section(Section::Browsers),
        4 => Page::Section(Section::Applications),
        5 => Page::Section(Section::Permissions),
        6 => Page::Section(Section::Sandbox),
        _ => Page::AllApps,
    }
}

fn page_title(p: &Page) -> String {
    match p {
        Page::Overview => "Overview".into(),
        Page::Section(s) => s.label().into(),
        Page::AllApps => "All installed apps".into(),
        Page::Category(c) => c.label().into(),
        Page::Search(q) => format!("Search: {q}"),
    }
}

fn source_label(a: &App) -> &'static str {
    match a.source {
        AppSource::Native => "",
        AppSource::Flatpak => " (Flatpak)",
        AppSource::Snap => " (Snap)",
        AppSource::System => "",
    }
}

fn app_icon(a: &App) -> gtk::Image {
    let img = icons::app_image(a.icon.as_deref(), 32);
    img.set_accessible_role(gtk::AccessibleRole::Presentation);
    img
}

impl Ui {
    pub fn new(app: &adw::Application, env: Env) -> Rc<Ui> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Privacy Panel")
            .default_width(1120)
            .default_height(780)
            .width_request(360)
            .height_request(480)
            .build();

        // Sidebar.
        let sidebar = gtk::ListBox::new();
        sidebar.add_css_class("navigation-sidebar");
        sidebar.update_property(&[gtk::accessible::Property::Label("Sections")]);
        let mut sidebar_pages = Vec::new();
        for (i, (label, icon)) in SIDEBAR.iter().enumerate() {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            b.set_margin_top(6);
            b.set_margin_bottom(6);
            let img = icons::image(icon);
            img.set_accessible_role(gtk::AccessibleRole::Presentation);
            let l = gtk::Label::new(Some(label));
            l.set_xalign(0.0);
            l.set_hexpand(true);
            let count = gtk::Label::new(None);
            count.add_css_class("dim-label");
            b.append(&img);
            b.append(&l);
            b.append(&count);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&b));
            sidebar.append(&row);
            sidebar_pages.push((sidebar_page(i), count));
        }
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search settings and apps"));
        search.update_property(&[gtk::accessible::Property::Label("Search settings and apps")]);
        search.set_margin_start(12);
        search.set_margin_end(12);
        search.set_margin_bottom(6);

        let side_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        side_box.append(&search);
        let side_scroll = gtk::ScrolledWindow::new();
        side_scroll.set_vexpand(true);
        side_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        side_scroll.set_child(Some(&sidebar));
        side_box.append(&side_scroll);

        let side_header = adw::HeaderBar::new();
        let menu = gio::Menu::new();
        menu.append(Some("History and undo"), Some("win.history"));
        menu.append(Some("Refresh"), Some("win.refresh"));
        menu.append(Some("About Privacy Panel"), Some("win.about"));
        let menu_btn = gtk::MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_menu_model(Some(&menu));
        menu_btn.set_tooltip_text(Some("Main menu"));
        menu_btn.update_property(&[gtk::accessible::Property::Label("Main menu")]);
        side_header.pack_end(&menu_btn);
        let spinner = gtk::Spinner::new();
        spinner.set_tooltip_text(Some("Working"));
        side_header.pack_start(&spinner);
        let side_tv = adw::ToolbarView::new();
        side_tv.add_top_bar(&side_header);
        side_tv.set_content(Some(&side_box));
        let side_page = adw::NavigationPage::new(&side_tv, "Privacy Panel");

        let nav = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&nav));
        let content_page = adw::NavigationPage::new(&toasts, "Content");

        let split = adw::NavigationSplitView::new();
        split.set_sidebar(Some(&side_page));
        split.set_content(Some(&content_page));
        split.set_min_sidebar_width(240.0);
        window.set_content(Some(&split));

        let bp = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 640sp").unwrap());
        bp.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(bp);

        let ui = Rc::new(Ui {
            env,
            model: RefCell::new(Model::default()),
            window,
            split,
            nav,
            sidebar,
            search,
            toasts,
            updaters: RefCell::new(HashMap::new()),
            page: RefCell::new(Page::Overview),
            sidebar_pages,
            busy: Cell::new(false),
            spinner,
        });
        ui.connect();
        ui.loading_page();
        ui
    }

    fn connect(self: &Rc<Self>) {
        let ui = self.clone();
        self.sidebar.connect_row_activated(move |_, row| {
            let i = row.index().max(0) as usize;
            ui.search.set_text("");
            ui.show(sidebar_page(i));
            ui.split.set_show_content(true);
        });
        let ui = self.clone();
        self.search.connect_search_changed(move |e| {
            let q = e.text().trim().to_string();
            if q.is_empty() {
                let i = ui.sidebar.selected_row().map(|r| r.index().max(0) as usize).unwrap_or(0);
                ui.show(sidebar_page(i));
            } else {
                ui.show(Page::Search(q));
                ui.split.set_show_content(true);
            }
        });

        let add = |name: &str, f: Box<dyn Fn()>| {
            let a = gio::SimpleAction::new(name, None);
            a.connect_activate(move |_, _| f());
            self.window.add_action(&a);
        };
        let ui = self.clone();
        add("refresh", Box::new(move || ui.reload()));
        let ui = self.clone();
        add("history", Box::new(move || crate::dialogs::history(&ui)));
        let ui = self.clone();
        add("about", Box::new(move || crate::dialogs::about(&ui)));
        let ui = self.clone();
        add("search", Box::new(move || {
            ui.split.set_show_content(false);
            ui.search.grab_focus();
        }));
        let ui = self.clone();
        add("review", Box::new(move || crate::dialogs::review(&ui, None)));
        if let Some(app) = self.window.application() {
            app.set_accels_for_action("win.refresh", &["F5", "<Control>r"]);
            app.set_accels_for_action("win.search", &["<Control>f"]);
            app.set_accels_for_action("win.history", &["<Control>h"]);
            app.set_accels_for_action("window.close", &["<Control>q", "<Control>w"]);
        }
    }

    fn set_busy(&self, b: bool) {
        self.busy.set(b);
        if b {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
    }

    fn loading_page(&self) {
        let sp = adw::StatusPage::builder()
            .title("Looking for apps and settings")
            .description("Reading the settings of the apps on this computer. Nothing is changed.")
            .icon_name("system-search-symbolic")
            .build();
        self.set_root(&sp.upcast(), "Privacy Panel");
    }

    /// Read everything again in the background.
    pub fn reload(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.set_busy(true);
        let env = self.env.clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let model = gio::spawn_blocking(move || privacy_core::providers::collect_all(&env))
                .await
                .unwrap_or_default();
            *ui.model.borrow_mut() = model;
            ui.set_busy(false);
            ui.update_counts();
            let p = ui.page.borrow().clone();
            ui.show(p);
            if ui.sidebar.selected_row().is_none() {
                if let Some(r) = ui.sidebar.row_at_index(0) {
                    ui.sidebar.select_row(Some(&r));
                }
            }
        });
    }

    fn update_counts(&self) {
        let m = self.model.borrow();
        for (page, label) in &self.sidebar_pages {
            let n = match page {
                Page::Overview => m.settings.iter().filter(|s| s.differs_from_recommended()).count(),
                Page::Section(sec) => m.settings.iter().filter(|s| s.section == *sec && s.differs_from_recommended()).count(),
                _ => 0,
            };
            label.set_text(&if n > 0 { n.to_string() } else { String::new() });
            let desc = if n > 0 { format!("{n} settings differ from the recommendation") } else { String::new() };
            if let Some(row) = label.ancestor(gtk::ListBoxRow::static_type()) {
                row.update_property(&[gtk::accessible::Property::Description(&desc)]);
            }
        }
    }

    pub fn register(&self, id: &str, f: Updater) {
        self.updaters.borrow_mut().entry(id.to_string()).or_default().push(f);
    }

    fn set_root(&self, child: &gtk::Widget, title: &str) {
        let page = self.wrap_page(child, title);
        self.nav.replace(&[page]);
    }

    fn wrap_page(&self, child: &gtk::Widget, title: &str) -> adw::NavigationPage {
        let tv = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(child));
        adw::NavigationPage::new(&tv, title)
    }

    pub fn show(self: &Rc<Self>, page: Page) {
        self.updaters.borrow_mut().clear();
        *self.page.borrow_mut() = page.clone();
        let w = match &page {
            Page::Overview => self.overview(),
            Page::Section(s) => self.section_page(*s),
            Page::AllApps => self.all_apps(),
            Page::Category(c) => self.category_page(*c),
            Page::Search(q) => self.search_page(q),
        };
        self.set_root(&w, &page_title(&page));
    }

    // ------------------------------------------------------------------ pages

    fn overview(self: &Rc<Self>) -> gtk::Widget {
        let m = self.model.borrow();
        let page = adw::PreferencesPage::new();
        let differ: Vec<&Setting> = m.settings.iter().filter(|s| s.differs_from_recommended()).collect();
        let editable = differ.iter().filter(|s| s.is_editable()).count();

        let head = adw::PreferencesGroup::new();
        let title = if differ.is_empty() {
            "Everything matches the recommendations".to_string()
        } else if differ.len() == 1 {
            "1 setting could be more private".to_string()
        } else {
            format!("{} settings could be more private", differ.len())
        };
        let hbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        hbox.set_margin_top(12);
        hbox.set_margin_bottom(12);
        let icon = gtk::Image::from_icon_name(if differ.is_empty() { "security-high-symbolic" } else { "security-medium-symbolic" });
        icon.set_pixel_size(64);
        icon.add_css_class("dim-label");
        icon.set_accessible_role(gtk::AccessibleRole::Presentation);
        let tl = gtk::Label::new(Some(&title));
        tl.add_css_class("title-1");
        tl.set_wrap(true);
        tl.set_justify(gtk::Justification::Center);
        tl.set_accessible_role(gtk::AccessibleRole::Heading);
        let dl = gtk::Label::new(Some(&format!(
            "Privacy Panel found {} apps and {} privacy settings on this computer. Nothing is changed until you change it, and every change can be undone from the menu.",
            m.apps.len(),
            m.settings.iter().filter(|s| matches!(s.kind, privacy_core::Kind::Bool | privacy_core::Kind::Choice(_))).count()
        )));
        dl.set_wrap(true);
        dl.set_justify(gtk::Justification::Center);
        hbox.append(&icon);
        hbox.append(&tl);
        hbox.append(&dl);
        if editable > 0 {
            let btn = gtk::Button::with_label(&format!("Review {editable} recommended changes"));
            btn.add_css_class("pill");
            btn.add_css_class("suggested-action");
            btn.set_halign(gtk::Align::Center);
            btn.set_action_name(Some("win.review"));
            hbox.append(&btn);
        }
        head.add(&hbox);
        page.add(&head);

        let cats = adw::PreferencesGroup::new();
        cats.set_title("By category");
        for c in Category::all() {
            let total = m.settings.iter().filter(|s| s.category == *c && s.recommended.is_some()).count();
            if total == 0 {
                continue;
            }
            let n = m.settings.iter().filter(|s| s.category == *c && s.differs_from_recommended()).count();
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            row.set_title(c.label());
            row.set_subtitle(&format!(
                "{}\n{}",
                privacy_core::about::category(*c),
                if n == 0 {
                    format!("All {total} settings match the recommendation.")
                } else {
                    format!("{n} of {total} settings differ from the recommendation.")
                }
            ));
            row.set_activatable(true);
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let ui = self.clone();
            let cat = *c;
            row.connect_activated(move |_| ui.show(Page::Category(cat)));
            cats.add(&row);
        }
        page.add(&cats);

        let blocked: Vec<String> = {
            let mut v: Vec<String> = m.settings.iter().filter_map(|s| s.blocked_by.clone()).collect();
            v.sort();
            v.dedup();
            v
        };
        if !blocked.is_empty() {
            let g = adw::PreferencesGroup::new();
            g.set_title("Running apps");
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            row.set_title("Some settings can only be changed while their app is closed");
            row.set_subtitle(&format!("Close {} and choose Refresh (F5).", blocked.join(", ")));
            let btn = gtk::Button::with_label("Refresh");
            btn.set_valign(gtk::Align::Center);
            btn.set_action_name(Some("win.refresh"));
            row.add_suffix(&btn);
            g.add(&row);
            page.add(&g);
        }

        let hist = adw::PreferencesGroup::new();
        let row = adw::ActionRow::new();
        row.set_title("History and undo");
        row.set_subtitle("See every change made with Privacy Panel, undo it, or put everything back as it was.");
        row.set_activatable(true);
        row.set_action_name(Some("win.history"));
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        hist.add(&row);
        page.add(&hist);
        page.upcast()
    }

    /// Groups of rows: one group per (app, sub-group), in app order.
    fn grouped(self: &Rc<Self>, page: &adw::PreferencesPage, settings: &[&Setting], show_apply: bool) {
        let m = self.model.borrow();
        let mut keys: Vec<(String, Option<String>)> = Vec::new();
        for s in settings {
            let k = (s.app_id.clone(), s.group.clone());
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys.sort_by_key(|(a, g)| {
            let name = m.app(a).map(|x| x.name.to_lowercase()).unwrap_or_default();
            let sys = m.app(a).is_some_and(|x| x.source == AppSource::System);
            // Within an app: ungrouped first, then profiles, policies for all users last.
            let gk = match g.as_deref() {
                None => 0,
                Some(x) if x.starts_with("Policies") => 3,
                Some(x) if x.starts_with("Whole browser") => 1,
                Some(_) => 2,
            };
            (!sys, name, gk, g.clone())
        });
        let mut last_app = String::new();
        for (app_id, group) in keys {
            let app = m.app(&app_id);
            let g = adw::PreferencesGroup::new();
            let name = app.map(|a| format!("{}{}", a.name, source_label(a))).unwrap_or_else(|| app_id.clone());
            g.set_title(&glib::markup_escape_text(&name));
            let mut desc: Vec<String> = Vec::new();
            if last_app != app_id {
                if let Some(a) = app.and_then(|a| a.about.clone().or_else(|| a.comment.clone())) {
                    desc.push(a);
                }
            }
            if let Some(gr) = &group {
                desc.push(gr.clone());
            }
            if !desc.is_empty() {
                g.set_description(Some(&glib::markup_escape_text(&desc.join("\n"))));
            }
            let mine: Vec<&&Setting> = settings.iter().filter(|s| s.app_id == app_id && s.group == group).collect();
            if show_apply && last_app != app_id {
                let n = m.settings_for_app(&app_id).filter(|s| s.is_editable() && s.differs_from_recommended()).count();
                if n > 0 {
                    let b = gtk::Button::with_label("Apply recommended");
                    b.add_css_class("flat");
                    b.set_valign(gtk::Align::Center);
                    b.update_property(&[gtk::accessible::Property::Label(&format!("Apply {n} recommended changes for {name}"))]);
                    let ui = self.clone();
                    let aid = app_id.clone();
                    b.connect_clicked(move |_| crate::dialogs::review(&ui, Some(aid.clone())));
                    g.set_header_suffix(Some(&b));
                }
            }
            last_app = app_id.clone();
            for s in mine {
                g.add(&rows::build(self, s, None));
            }
            page.add(&g);
        }
    }

    fn section_page(self: &Rc<Self>, sec: Section) -> gtk::Widget {
        let settings: Vec<Setting> = self.model.borrow().settings.iter().filter(|s| s.section == sec).cloned().collect();
        if settings.is_empty() {
            let (title, desc) = match sec {
                Section::Permissions => ("No permissions granted", "No app has been given camera, microphone, location or similar access through the desktop's permission prompts."),
                Section::Sandbox => ("No sandboxed apps", "No Flatpak or Snap apps are installed."),
                Section::Browsers => ("No known browsers", "None of the browsers Privacy Panel knows are installed."),
                _ => ("Nothing found", "Privacy Panel found no settings of this kind on this computer."),
            };
            return adw::StatusPage::builder().title(title).description(desc).icon_name(icons::resolve(icons::OK)).build().upcast();
        }
        let page = adw::PreferencesPage::new();
        let intro = adw::PreferencesGroup::new();
        intro.set_description(Some(&glib::markup_escape_text(&format!(
            "{} Choose the \u{24d8} button on a row to see what it is and exactly what changes.",
            privacy_core::about::section(sec)
        ))));
        page.add(&intro);
        let refs: Vec<&Setting> = settings.iter().collect();
        self.grouped(&page, &refs, true);
        page.upcast()
    }

    fn category_page(self: &Rc<Self>, c: Category) -> gtk::Widget {
        let settings: Vec<Setting> = self.model.borrow().settings.iter().filter(|s| s.category == c).cloned().collect();
        let page = adw::PreferencesPage::new();
        let intro = adw::PreferencesGroup::new();
        intro.set_description(Some(privacy_core::about::category(c)));
        page.add(&intro);
        let refs: Vec<&Setting> = settings.iter().collect();
        self.grouped(&page, &refs, false);
        page.upcast()
    }

    fn search_page(self: &Rc<Self>, q: &str) -> gtk::Widget {
        let ql = q.to_lowercase();
        let (settings, apps): (Vec<Setting>, Vec<App>) = {
            let m = self.model.borrow();
            let s = m
                .settings
                .iter()
                .filter(|s| {
                    let app = m.app(&s.app_id).map(|a| a.name.to_lowercase()).unwrap_or_default();
                    s.title.to_lowercase().contains(&ql)
                        || s.explanation.to_lowercase().contains(&ql)
                        || app.contains(&ql)
                        || s.group.as_ref().is_some_and(|g| g.to_lowercase().contains(&ql))
                })
                .cloned()
                .collect();
            let a = m.apps.iter().filter(|a| a.name.to_lowercase().contains(&ql)).cloned().collect();
            (s, a)
        };
        if settings.is_empty() && apps.is_empty() {
            return adw::StatusPage::builder()
                .title("No results")
                .description(format!("Nothing matches \"{q}\"."))
                .icon_name("edit-find-symbolic")
                .build()
                .upcast();
        }
        let page = adw::PreferencesPage::new();
        if !apps.is_empty() {
            let g = adw::PreferencesGroup::new();
            g.set_title("Apps");
            for a in &apps {
                g.add(&self.app_row(a));
            }
            page.add(&g);
        }
        let refs: Vec<&Setting> = settings.iter().collect();
        self.grouped(&page, &refs, false);
        page.upcast()
    }

    fn app_row(self: &Rc<Self>, a: &App) -> adw::ActionRow {
        let m = self.model.borrow();
        let n = m.settings_for_app(&a.id).count();
        let differ = m.settings_for_app(&a.id).filter(|s| s.differs_from_recommended()).count();
        let row = adw::ActionRow::new();
        row.set_use_markup(false);
        row.set_title(&format!("{}{}", a.name, source_label(a)));
        let mut sub = match n {
            0 => "No known privacy settings".to_string(),
            1 => "1 privacy setting".to_string(),
            n => format!("{n} privacy settings"),
        };
        if differ > 0 {
            sub.push_str(&format!(", {differ} differ from the recommendation"));
        }
        row.set_subtitle(&sub);
        row.add_prefix(&app_icon(a));
        row.set_activatable(true);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let ui = self.clone();
        let id = a.id.clone();
        row.connect_activated(move |_| ui.push_app(&id));
        row
    }

    fn all_apps(self: &Rc<Self>) -> gtk::Widget {
        let apps: Vec<App> = self.model.borrow().apps.iter().filter(|a| a.source != AppSource::System).cloned().collect();
        let page = adw::PreferencesPage::new();
        let with: Vec<&App> = apps.iter().filter(|a| self.model.borrow().settings_for_app(&a.id).next().is_some()).collect();
        let without: Vec<&App> = apps.iter().filter(|a| self.model.borrow().settings_for_app(&a.id).next().is_none()).collect();
        let g = adw::PreferencesGroup::new();
        g.set_title("Apps with privacy settings");
        g.set_description(Some("Choose an app to see all its settings and permissions."));
        for a in &with {
            g.add(&self.app_row(a));
        }
        page.add(&g);
        let g = adw::PreferencesGroup::new();
        g.set_title(&format!("Other installed apps ({})", without.len()));
        g.set_description(Some("Privacy Panel knows no privacy settings for these apps. It only shows settings it can really read and change, and never guesses."));
        for a in &without {
            g.add(&self.app_row(a));
        }
        page.add(&g);
        page.upcast()
    }

    pub fn push_app(self: &Rc<Self>, id: &str) {
        let (app, settings) = {
            let m = self.model.borrow();
            let Some(app) = m.app(id).cloned() else { return };
            (app, m.settings_for_app(id).cloned().collect::<Vec<_>>())
        };
        let w: gtk::Widget = if settings.is_empty() {
            let fallback = match app.source {
                AppSource::Native => "Privacy Panel knows no privacy settings for this app. It is not sandboxed, so like any program you run it can read your files and use the network.",
                AppSource::Flatpak => "This Flatpak app asks for no sandbox permissions that Privacy Panel shows.",
                _ => "Privacy Panel knows no privacy settings for this app.",
            };
            let desc = match (&app.about, &app.comment) {
                (Some(a), _) => format!("{a}\n\n{fallback}"),
                (None, Some(c)) => format!("{c}\n\n{fallback}"),
                _ => fallback.to_string(),
            };
            let sp = adw::StatusPage::builder()
                .title(format!("{}{}", app.name, source_label(&app)))
                .description(glib::markup_escape_text(&desc).to_string())
                .build();
            sp.set_paintable(icons::app_paintable(app.icon.as_deref(), 128, &self.window).as_ref());
            sp.upcast()
        } else {
            let page = adw::PreferencesPage::new();
            let refs: Vec<&Setting> = settings.iter().collect();
            self.grouped(&page, &refs, true);
            page.upcast()
        };
        let p = self.wrap_page(&w, &app.name);
        self.nav.push(&p);
    }

    // ------------------------------------------------------------------ changes

    /// The user changed a row.
    pub fn change(self: &Rc<Self>, id: &str, v: Value) {
        if self.busy.get() {
            // One change at a time: put the row back and say why.
            self.toast("Please wait until the previous change is finished.");
            self.refresh_rows(&[id.to_string()]);
            return;
        }
        let planned = {
            let m = self.model.borrow();
            let Some(s) = m.setting(id) else { return };
            if s.current.value() == Some(&v) {
                return;
            }
            engine::plan(&m, s, &v)
        };
        match planned {
            Ok(c) => self.apply(vec![c], None),
            Err(e) => {
                self.show_errors(&[(String::new(), e)]);
                self.refresh_rows(&[id.to_string()]);
            }
        }
    }

    /// Apply changes in the background, then re-read what changed.
    pub fn apply(self: &Rc<Self>, changes: Vec<PlannedChange>, done: Option<Box<dyn Fn()>>) {
        if changes.is_empty() || self.busy.get() {
            return;
        }
        self.set_busy(true);
        let env = self.env.clone();
        let ids: Vec<String> = changes.iter().map(|c| c.setting_id.clone()).collect();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let rep: Report = gio::spawn_blocking(move || engine::apply(&env, &changes, &PkexecHelper, None))
                .await
                .unwrap_or_else(|_| Report { errors: vec![("".into(), "The change crashed.".into())], ..Default::default() });
            ui.after_change(&ids, rep).await;
            if let Some(f) = done {
                f();
            }
        });
    }

    pub async fn after_change(self: &Rc<Self>, ids: &[String], rep: Report) {
        self.reread(ids).await;
        self.set_busy(false);
        if !rep.errors.is_empty() {
            self.show_errors(&rep.errors);
        }
        if !rep.applied.is_empty() {
            let msg = if rep.applied.len() == 1 {
                let m = self.model.borrow();
                let s = m.setting(&rep.applied[0]);
                s.map(|s| format!("Changed \u{201c}{}\u{201d} to {}", s.title, s.current_label()))
                    .unwrap_or_else(|| "Changed 1 setting".into())
            } else {
                format!("Changed {} settings", rep.applied.len())
            };
            let toast = adw::Toast::new(&msg);
            toast.set_button_label(Some("Undo"));
            let ui = self.clone();
            let batch = rep.batch;
            toast.connect_button_clicked(move |_| ui.undo(batch));
            self.toasts.add_toast(toast);
        }
    }

    /// Re-read some settings (in the background) and update their rows.
    pub async fn reread(self: &Rc<Self>, ids: &[String]) {
        let settings: Vec<Setting> = {
            let m = self.model.borrow();
            ids.iter().filter_map(|i| m.setting(i).cloned()).collect()
        };
        let env = self.env.clone();
        let fresh = gio::spawn_blocking(move || {
            let running = privacy_core::discovery::running_processes(&env);
            settings
                .into_iter()
                .map(|mut s| {
                    s.current = engine::read_current(&env, &s);
                    if !s.requires_closed.iter().any(|p| running.contains(p)) {
                        s.blocked_by = None;
                    }
                    s
                })
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        {
            let mut m = self.model.borrow_mut();
            for s in &fresh {
                if let Some(x) = m.setting_mut(&s.id) {
                    x.current = s.current.clone();
                    x.blocked_by = s.blocked_by.clone();
                }
            }
        }
        self.refresh_rows(ids);
        self.update_counts();
    }

    pub fn refresh_rows(&self, ids: &[String]) {
        let m = self.model.borrow();
        let ups = self.updaters.borrow();
        for id in ids {
            if let (Some(s), Some(list)) = (m.setting(id), ups.get(id)) {
                for f in list {
                    f(s);
                }
            }
        }
    }

    pub fn undo(self: &Rc<Self>, batch: u64) {
        if self.busy.get() {
            self.toast("Please wait until the previous change is finished.");
            return;
        }
        self.set_busy(true);
        let env = self.env.clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let ids: Vec<String> = privacy_core::history::batches(&env)
                .into_iter()
                .find(|b| b.id == batch)
                .map(|b| b.entries.iter().map(|e| e.setting_id.clone()).collect())
                .unwrap_or_default();
            let env2 = env.clone();
            let rep = gio::spawn_blocking(move || engine::undo_batch(&env2, batch, &PkexecHelper))
                .await
                .unwrap_or_default();
            ui.reread(&ids).await;
            ui.set_busy(false);
            if rep.errors.is_empty() {
                ui.toasts.add_toast(adw::Toast::new("Undone"));
            } else {
                ui.show_errors(&rep.errors);
            }
        });
    }

    pub fn restore_everything(self: &Rc<Self>) {
        if self.busy.get() {
            self.toast("Please wait until the previous change is finished.");
            return;
        }
        self.set_busy(true);
        let env = self.env.clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let rep = gio::spawn_blocking(move || engine::restore_everything(&env, &PkexecHelper))
                .await
                .unwrap_or_default();
            ui.set_busy(false);
            if rep.errors.is_empty() {
                ui.toasts.add_toast(adw::Toast::new("Everything is back as it was"));
            } else {
                ui.show_errors(&rep.errors);
            }
            ui.reload();
        });
    }

    pub fn show_errors(&self, errors: &[(String, String)]) {
        let body = errors
            .iter()
            .map(|(t, e)| if t.is_empty() { e.clone() } else { format!("{t}: {e}") })
            .collect::<Vec<_>>()
            .join("\n");
        let d = adw::AlertDialog::new(
            Some(if errors.len() == 1 { "The change could not be made" } else { "Some changes could not be made" }),
            Some(&body),
        );
        d.add_response("ok", "OK");
        d.set_default_response(Some("ok"));
        d.set_close_response("ok");
        d.present(Some(&self.window));
    }

    pub fn confirm_action(self: &Rc<Self>, kind: ActionKind, title: &str, expl: &str, button: &str) {
        let d = adw::AlertDialog::new(Some(title), Some(expl));
        d.add_response("cancel", "Cancel");
        d.add_response("go", button);
        d.set_response_appearance("go", adw::ResponseAppearance::Destructive);
        d.set_default_response(Some("cancel"));
        d.set_close_response("cancel");
        let ui = self.clone();
        d.connect_response(None, move |_, r| {
            if r != "go" || ui.busy.get() {
                return;
            }
            let env = ui.env.clone();
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(move || engine::run_action(&env, kind))
                    .await
                    .unwrap_or_else(|_| Err("crashed".into()));
                match res {
                    Ok(msg) => ui.toasts.add_toast(adw::Toast::new(&msg)),
                    Err(e) => ui.show_errors(&[(String::new(), e)]),
                }
            });
        });
        d.present(Some(&self.window));
    }

    pub fn current_page(&self) -> Page {
        self.page.borrow().clone()
    }

    pub fn toast(&self, msg: &str) {
        self.toasts.add_toast(adw::Toast::new(msg));
    }
}
