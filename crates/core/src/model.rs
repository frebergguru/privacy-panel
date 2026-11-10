//! The data the GUI shows: apps and their settings.

use serde::{Deserialize, Serialize};

use crate::target::Target;

/// A value as the user sees it: a switch position or the id of a choice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Choice(String),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Telemetry,
    Tracking,
    History,
    Location,
    Network,
    Sharing,
    Sandbox,
    Updates,
    Security,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Telemetry => "Telemetry and data collection",
            Category::Tracking => "Tracking",
            Category::History => "History and recent files",
            Category::Location => "Location",
            Category::Network => "Network exposure",
            Category::Sharing => "Sharing and devices",
            Category::Sandbox => "Sandbox permissions",
            Category::Updates => "Update and online checks",
            Category::Security => "Security",
        }
    }
    pub fn all() -> &'static [Category] {
        &[
            Category::Telemetry,
            Category::Tracking,
            Category::History,
            Category::Location,
            Category::Network,
            Category::Sharing,
            Category::Sandbox,
            Category::Updates,
            Category::Security,
        ]
    }
}

/// Which sidebar page a setting belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    System,
    Network,
    Browsers,
    Applications,
    Permissions,
    Sandbox,
}

impl Section {
    pub fn label(&self) -> &'static str {
        match self {
            Section::System => "System",
            Section::Network => "Network",
            Section::Browsers => "Browsers",
            Section::Applications => "Applications",
            Section::Permissions => "App permissions",
            Section::Sandbox => "Sandboxed apps",
        }
    }
    pub fn all() -> &'static [Section] {
        &[
            Section::System,
            Section::Network,
            Section::Browsers,
            Section::Applications,
            Section::Permissions,
            Section::Sandbox,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceOpt {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    ClearThumbnails,
    ClearRecentFiles,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Bool,
    Choice(Vec<ChoiceOpt>),
    /// Read-only information.
    Info,
    /// A one-off action such as clearing a cache. Cannot be undone.
    Action(ActionKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    User,
    Admin,
}

/// What the setting is set to right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Current {
    /// A known value. `is_default` is true when nothing is set and the app's default applies.
    Value { value: Value, is_default: bool },
    /// Set to something this program does not know; the raw text is shown.
    Other(String),
    /// Could not be read; the reason is shown.
    Unavailable(String),
    /// Text for an information row.
    Info(String),
}

impl Current {
    pub fn value(&self) -> Option<&Value> {
        match self {
            Current::Value { value, .. } => Some(value),
            _ => None,
        }
    }
}

/// How a user-facing value maps to the raw text stored by the app.
/// A raw of `None` means "the key is absent".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueMap {
    pub entries: Vec<(Value, Option<String>)>,
    /// Extra raw values that read as a given value (never written).
    pub aliases: Vec<(String, Value)>,
}

impl ValueMap {
    pub fn bool(on: &str, off: &str) -> ValueMap {
        ValueMap {
            entries: vec![
                (Value::Bool(true), Some(on.to_string())),
                (Value::Bool(false), Some(off.to_string())),
            ],
            aliases: vec![],
        }
    }
    /// A switch where "on" writes `on` and "off" removes the key.
    pub fn bool_or_absent(on: &str) -> ValueMap {
        ValueMap {
            entries: vec![
                (Value::Bool(true), Some(on.to_string())),
                (Value::Bool(false), None),
            ],
            aliases: vec![],
        }
    }
    pub fn alias(mut self, raw: &str, v: Value) -> ValueMap {
        self.aliases.push((raw.to_string(), v));
        self
    }
    pub fn raw_for(&self, v: &Value) -> Option<Option<String>> {
        self.entries.iter().find(|(e, _)| e == v).map(|(_, r)| r.clone())
    }
    /// All raw values that may ever be written.
    pub fn raws(&self) -> Vec<Option<String>> {
        self.entries.iter().map(|(_, r)| r.clone()).collect()
    }
}

/// How to read and write one setting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub target: Target,
    pub map: ValueMap,
    /// The value that applies when the key is absent and the map has no `None` entry.
    pub default: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setting {
    /// Unique and stable, e.g. "firefox@abc.default.telemetry".
    pub id: String,
    pub app_id: String,
    /// Sub-heading inside the app, e.g. a browser profile name.
    pub group: Option<String>,
    pub title: String,
    pub explanation: String,
    /// What the thing is, in plain words ("What is this?").
    #[serde(default)]
    pub about: Option<String>,
    pub category: Category,
    pub section: Section,
    pub kind: Kind,
    pub recommended: Option<Value>,
    /// Process names that must not be running while this is changed.
    pub requires_closed: Vec<String>,
    pub spec: Spec,
    pub current: Current,
    /// Set when one of `requires_closed` is running: the app's name.
    pub blocked_by: Option<String>,
}

impl Setting {
    pub fn scope(&self) -> Scope {
        if self.spec.target.is_admin() {
            Scope::Admin
        } else {
            Scope::User
        }
    }

    pub fn is_editable(&self) -> bool {
        matches!(self.kind, Kind::Bool | Kind::Choice(_))
            && self.blocked_by.is_none()
            && !matches!(self.current, Current::Unavailable(_))
    }

    /// True when the current value is known and differs from the recommended one.
    pub fn differs_from_recommended(&self) -> bool {
        match (&self.recommended, &self.current) {
            (Some(r), Current::Value { value, .. }) => r != value,
            (Some(_), Current::Other(_)) => true,
            _ => false,
        }
    }

    pub fn value_label(&self, v: &Value) -> String {
        match (v, &self.kind) {
            (Value::Bool(true), _) => "On".into(),
            (Value::Bool(false), _) => "Off".into(),
            (Value::Choice(id), Kind::Choice(opts)) => opts
                .iter()
                .find(|o| &o.id == id)
                .map(|o| o.label.clone())
                .unwrap_or_else(|| id.clone()),
            (Value::Choice(id), _) => id.clone(),
        }
    }

    /// Plain-words description of the current state.
    pub fn current_label(&self) -> String {
        match &self.current {
            Current::Value { value, is_default } => {
                let l = self.value_label(value);
                if *is_default {
                    format!("{l} (default)")
                } else {
                    l
                }
            }
            Current::Other(raw) => format!("Custom value: {raw}"),
            Current::Unavailable(why) => why.clone(),
            Current::Info(t) => t.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppSource {
    Native,
    Flatpak,
    Snap,
    /// A part of the desktop or operating system rather than an installed app.
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub source: AppSource,
    pub desktop_ids: Vec<String>,
    pub processes: Vec<String>,
    /// Flatpak or snap id, if any.
    pub sandbox_id: Option<String>,
    /// What the app is, in plain words.
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Model {
    pub apps: Vec<App>,
    pub settings: Vec<Setting>,
}

impl Model {
    pub fn app(&self, id: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.id == id)
    }
    pub fn settings_for_app<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Setting> + 'a {
        self.settings.iter().filter(move |s| s.app_id == id)
    }
    pub fn setting(&self, id: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.id == id)
    }
    pub fn setting_mut(&mut self, id: &str) -> Option<&mut Setting> {
        self.settings.iter_mut().find(|s| s.id == id)
    }
}
