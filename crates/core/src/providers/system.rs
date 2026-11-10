//! System-wide settings from `admin::admin_defs()`. Changing them needs the root helper.

use super::{system_app, Collector};
use crate::admin::{admin_defs, AdminTarget, Condition};
use crate::backends::system;
use crate::model::{Current, Setting, Spec};
use crate::target::Target;

fn app_info(id: &str) -> (&'static str, &'static str, &'static str) {
    match id {
        "system-services" => ("System services", "preferences-system", "Background services that run for the whole computer"),
        "networkmanager" => ("NetworkManager", "network-wireless", "How this computer appears on networks"),
        "system-logs" => ("System logs and crash dumps", "utilities-log-viewer", "What the system writes down about itself"),
        "distro-telemetry" => ("Distribution telemetry", "start-here", "Data sent to the makers of your Linux distribution"),
        _ => ("System", "preferences-system", ""),
    }
}

/// The distribution's logo icon name, from `LOGO=` in os-release.
fn distro_logo(env: &crate::env::Env) -> Option<String> {
    ["/etc/os-release", "/usr/lib/os-release"].iter().find_map(|p| {
        let text = std::fs::read_to_string(env.sys(p)).ok()?;
        text.lines().find_map(|l| l.strip_prefix("LOGO=")).map(|v| v.trim().trim_matches(['"', '\'']).to_string()).filter(|v| !v.is_empty())
    })
}

pub fn collect(c: &mut Collector) {
    for d in admin_defs() {
        let show = match &d.condition {
            Condition::Always => true,
            Condition::UnitExists => match &d.target {
                AdminTarget::Unit { unit } => system::unit_state(c.env, unit).is_some(),
                _ => true,
            },
            Condition::FileExists(p) => c.env.sys(p).exists(),
            Condition::Binary(list) => list.iter().any(|b| c.env.has_binary(b)) && (d.section != crate::model::Section::Browsers || c.has_app(d.app)),
        };
        if !show {
            continue;
        }
        if !c.has_app(d.app) {
            let (name, icon, comment) = app_info(d.app);
            let logo = if d.app == "distro-telemetry" { distro_logo(c.env) } else { None };
            c.app(system_app(d.app, name, logo.as_deref().unwrap_or(icon), comment));
        }
        let group = match &d.target {
            AdminTarget::Json { .. } => Some("Policies for all users".to_string()),
            _ => None,
        };
        c.add(Setting {
            id: d.id.clone(),
            app_id: d.app.to_string(),
            group,
            title: d.title.clone(),
            explanation: d.explanation.clone(),
            about: None,
            category: d.category,
            section: d.section,
            kind: d.kind.clone(),
            recommended: d.recommended.clone(),
            requires_closed: vec![],
            spec: Spec { target: Target::Admin(d.target.clone()), map: d.map.clone(), default: d.default.clone() },
            current: Current::Unavailable("Not read yet".into()),
            blocked_by: None,
        });
    }
}
