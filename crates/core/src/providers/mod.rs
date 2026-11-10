//! Providers find apps and describe their settings. Reading the current values is done
//! afterwards by `engine::refresh`, so providers only say where things live.

use std::collections::HashSet;

use crate::discovery::{self, Inventory};
use crate::env::Env;
use crate::model::{
    ActionKind, App, AppSource, Category, ChoiceOpt, Current, Kind, Model, Section, Setting, Spec, Value, ValueMap,
};
use crate::target::Target;

pub mod browsers;
pub mod declarative;
pub mod desktop;
pub mod sandbox;
pub mod system;

pub struct Collector<'a> {
    pub env: &'a Env,
    pub inv: &'a Inventory,
    pub apps: Vec<App>,
    pub settings: Vec<Setting>,
    claimed: HashSet<String>,
}

impl<'a> Collector<'a> {
    pub fn new(env: &'a Env, inv: &'a Inventory) -> Self {
        Collector { env, inv, apps: vec![], settings: vec![], claimed: HashSet::new() }
    }

    /// Add an app, or merge desktop ids into an existing one with the same id.
    pub fn app(&mut self, app: App) {
        for d in &app.desktop_ids {
            self.claimed.insert(d.clone());
        }
        if let Some(sid) = &app.sandbox_id {
            self.claimed.insert(sid.clone());
        }
        if let Some(a) = self.apps.iter_mut().find(|a| a.id == app.id) {
            for d in app.desktop_ids {
                if !a.desktop_ids.contains(&d) {
                    a.desktop_ids.push(d);
                }
            }
            return;
        }
        self.apps.push(app);
    }

    pub fn has_app(&self, id: &str) -> bool {
        self.apps.iter().any(|a| a.id == id)
    }

    pub fn add(&mut self, s: Setting) {
        if !self.settings.iter().any(|x| x.id == s.id) {
            self.settings.push(s);
        }
    }

    /// Build an app for a known program, taking name and icon from its desktop file
    /// when one of `desktop_ids` is installed.
    pub fn known_app(&self, id: &str, name: &str, icon: &str, desktop_ids: &[&str], processes: &[&str]) -> App {
        let found: Vec<String> = desktop_ids
            .iter()
            .filter(|d| self.inv.has_desktop(d))
            .map(|d| d.to_string())
            .collect();
        let entry = found.first().and_then(|d| self.inv.desktop.get(d));
        App {
            id: id.to_string(),
            name: name.to_string(),
            icon: entry.and_then(|e| e.icon.clone()).or_else(|| Some(icon.to_string())),
            comment: entry.and_then(|e| e.comment.clone()),
            source: AppSource::Native,
            desktop_ids: found,
            processes: processes.iter().map(|s| s.to_string()).collect(),
            sandbox_id: None,
            about: None,
        }
    }

    pub fn is_claimed(&self, desktop_id: &str) -> bool {
        self.claimed.contains(desktop_id)
    }
}

pub fn system_app(id: &str, name: &str, icon: &str, comment: &str) -> App {
    App {
        id: id.into(),
        name: name.into(),
        icon: Some(icon.into()),
        comment: Some(comment.into()),
        source: AppSource::System,
        desktop_ids: vec![],
        processes: vec![],
        sandbox_id: None,
        about: None,
    }
}

/// Start a setting; finish it with `.boolean()`, `.choice()`, `.info()` or `.action()`.
pub fn setting(
    id: impl Into<String>,
    app: &str,
    section: Section,
    category: Category,
    title: impl Into<String>,
    explanation: impl Into<String>,
) -> Setting {
    Setting {
        id: id.into(),
        app_id: app.into(),
        group: None,
        title: title.into(),
        explanation: explanation.into(),
        about: None,
        category,
        section,
        kind: Kind::Info,
        recommended: None,
        requires_closed: vec![],
        spec: Spec { target: Target::None, map: ValueMap::default(), default: None },
        current: Current::Info(String::new()),
        blocked_by: None,
    }
}

impl Setting {
    pub fn boolean(mut self, target: Target, map: ValueMap, default: Option<bool>, recommended: Option<bool>) -> Self {
        self.kind = Kind::Bool;
        self.spec = Spec { target, map, default: default.map(Value::Bool) };
        self.recommended = recommended.map(Value::Bool);
        self.current = Current::Unavailable("Not read yet".into());
        self
    }

    /// `opts` are (id, label, raw); a raw of None means "key absent".
    pub fn choice(
        mut self,
        target: Target,
        opts: &[(&str, &str, Option<&str>)],
        default: Option<&str>,
        recommended: Option<&str>,
    ) -> Self {
        self.kind = Kind::Choice(
            opts.iter()
                .map(|(id, label, _)| ChoiceOpt { id: id.to_string(), label: label.to_string() })
                .collect(),
        );
        self.spec = Spec {
            target,
            map: ValueMap {
                entries: opts
                    .iter()
                    .map(|(id, _, raw)| (Value::Choice(id.to_string()), raw.map(String::from)))
                    .collect(),
                aliases: vec![],
            },
            default: default.map(|d| Value::Choice(d.to_string())),
        };
        self.recommended = recommended.map(|r| Value::Choice(r.to_string()));
        self.current = Current::Unavailable("Not read yet".into());
        self
    }

    pub fn info(mut self, text: impl Into<String>) -> Self {
        self.kind = Kind::Info;
        self.current = Current::Info(text.into());
        self
    }

    pub fn action(mut self, kind: ActionKind, button: &str) -> Self {
        self.kind = Kind::Action(kind);
        self.current = Current::Info(button.into());
        self
    }

    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    pub fn closed(mut self, procs: &[String]) -> Self {
        self.requires_closed = procs.to_vec();
        self
    }

    pub fn alias(mut self, raw: &str, v: Value) -> Self {
        self.spec.map.aliases.push((raw.into(), v));
        self
    }
}

/// Find everything and read every current value.
pub fn collect_all(env: &Env) -> Model {
    let inv = discovery::scan(env);
    let mut c = Collector::new(env, &inv);
    desktop::collect(&mut c);
    browsers::collect(&mut c);
    declarative::collect(&mut c);
    sandbox::collect(&mut c);
    system::collect(&mut c);

    // Every other installed app, so the full list is complete.
    let rest: Vec<App> = inv
        .desktop
        .values()
        .filter(|d| !d.no_display && !c.is_claimed(&d.id))
        .filter(|d| d.flatpak.as_ref().is_none_or(|f| !c.is_claimed(f)))
        .map(discovery::app_from_desktop)
        .collect();
    for a in rest {
        c.app(a);
    }

    let mut model = Model { apps: c.apps, settings: c.settings };
    for s in &mut model.settings {
        if s.about.is_none() {
            let mut text = crate::about::setting(&s.id).map(String::from);
            if matches!(s.spec.target, crate::target::Target::Admin(crate::admin::AdminTarget::Json { .. })) {
                text = Some(match text {
                    Some(t) => format!("{t}\n\n{}", crate::about::POLICY_NOTE),
                    None => crate::about::POLICY_NOTE.to_string(),
                });
            }
            s.about = text;
        }
    }
    for a in &mut model.apps {
        if a.about.is_none() {
            a.about = crate::about::app(&a.id).or_else(|| crate::about::app_by_source(a.source)).map(String::from);
        }
    }
    crate::engine::refresh(env, &mut model);
    model.apps.sort_by_key(|a| a.name.to_lowercase());
    model
}
