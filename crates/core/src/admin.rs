//! System-wide settings that need root. The table in `admin_defs()` is the single source of
//! truth: the GUI builds its rows from it, and the root helper refuses any request that is
//! not exactly one of these targets with one of these values.

use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::backends::{ini, jsonc, system};
use crate::env::Env;
use crate::model::{Category, ChoiceOpt, Kind, Section, Value, ValueMap};
use crate::target::{read_file, write_file};

/// Files the helper may write. Paths are fixed here and never come from a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManagedFile {
    NetworkManager,
    Journald,
    Coredump,
    FirefoxPolicies,
    ChromePolicies,
    ChromiumPolicies,
    BravePolicies,
    EdgePolicies,
    Popcon,
    MotdNews,
    Apport,
    Whoopsie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    Ini,
    Json,
}

impl ManagedFile {
    pub fn path(&self) -> &'static str {
        match self {
            ManagedFile::NetworkManager => "/etc/NetworkManager/conf.d/90-privacy-panel.conf",
            ManagedFile::Journald => "/etc/systemd/journald.conf.d/90-privacy-panel.conf",
            ManagedFile::Coredump => "/etc/systemd/coredump.conf.d/90-privacy-panel.conf",
            ManagedFile::FirefoxPolicies => "/etc/firefox/policies/policies.json",
            ManagedFile::ChromePolicies => "/etc/opt/chrome/policies/managed/privacy-panel.json",
            ManagedFile::ChromiumPolicies => "/etc/chromium/policies/managed/privacy-panel.json",
            ManagedFile::BravePolicies => "/etc/brave/policies/managed/privacy-panel.json",
            ManagedFile::EdgePolicies => "/etc/opt/edge/policies/managed/privacy-panel.json",
            ManagedFile::Popcon => "/etc/popularity-contest.conf",
            ManagedFile::MotdNews => "/etc/default/motd-news",
            ManagedFile::Apport => "/etc/default/apport",
            ManagedFile::Whoopsie => "/etc/whoopsie",
        }
    }

    pub fn format(&self) -> FileFormat {
        match self {
            ManagedFile::FirefoxPolicies
            | ManagedFile::ChromePolicies
            | ManagedFile::ChromiumPolicies
            | ManagedFile::BravePolicies
            | ManagedFile::EdgePolicies => FileFormat::Json,
            _ => FileFormat::Ini,
        }
    }

    /// Distribution files we edit in place rather than create.
    pub fn must_exist(&self) -> bool {
        matches!(
            self,
            ManagedFile::Popcon | ManagedFile::MotdNews | ManagedFile::Apport | ManagedFile::Whoopsie
        )
    }

    /// Other files that can hold the same keys, read in order (later wins), before ours.
    fn read_order(&self, env: &Env) -> Vec<std::path::PathBuf> {
        let glob_dir = |d: &str| -> Vec<std::path::PathBuf> {
            let mut v: Vec<_> = std::fs::read_dir(env.sys(d))
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|x| x == "conf"))
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            v
        };
        let ours = env.sys(self.path());
        let mut out = match self {
            ManagedFile::NetworkManager => {
                // NetworkManager.conf first, then all conf.d files sorted by name.
                let mut d = glob_dir("/usr/lib/NetworkManager/conf.d");
                d.extend(glob_dir("/etc/NetworkManager/conf.d"));
                if !d.contains(&ours) {
                    d.push(ours.clone());
                }
                d.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));
                let mut v = vec![env.sys("/etc/NetworkManager/NetworkManager.conf")];
                v.extend(d);
                v
            }
            ManagedFile::Journald => {
                let mut d = glob_dir("/etc/systemd/journald.conf.d");
                if !d.contains(&ours) {
                    d.push(ours.clone());
                }
                d.sort();
                let mut v = vec![env.sys("/etc/systemd/journald.conf")];
                v.extend(d);
                v
            }
            ManagedFile::Coredump => {
                let mut d = glob_dir("/etc/systemd/coredump.conf.d");
                if !d.contains(&ours) {
                    d.push(ours.clone());
                }
                d.sort();
                let mut v = vec![env.sys("/etc/systemd/coredump.conf")];
                v.extend(d);
                v
            }
            _ => vec![],
        };
        // Our own drop-in takes its sorted place among the others (last one wins).
        if !out.contains(&ours) {
            out.push(ours);
        }
        out
    }

    /// Command run after a change, if any.
    fn post_action(&self) -> Option<&'static [&'static str]> {
        match self {
            ManagedFile::NetworkManager => Some(&["reload", "NetworkManager.service"]),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdminTarget {
    Ini { file: ManagedFile, group: String, key: String },
    Json { file: ManagedFile, pointer: Vec<String> },
    Unit { unit: String },
}

impl AdminTarget {
    pub fn read(&self, env: &Env) -> Result<Option<String>, String> {
        match self {
            AdminTarget::Ini { file, group, key } => {
                // The value in effect: every file in precedence order, last one wins.
                let mut found = None;
                for p in file.read_order(env) {
                    if let Some(v) = ini::get(&read_file(&p).unwrap_or_default(), group, key) {
                        found = Some(v);
                    }
                }
                Ok(found)
            }
            AdminTarget::Json { file, pointer } => jsonc::get(&read_file(&env.sys(file.path()))?, pointer),
            AdminTarget::Unit { unit } => Ok(system::unit_state(env, unit)),
        }
    }

    /// Only our own managed file (or the unit state, normalised to a value the helper
    /// accepts). Used for undo snapshots, so undo never writes values borrowed from
    /// other files.
    pub fn read_own(&self, env: &Env) -> Result<Option<String>, String> {
        match self {
            AdminTarget::Ini { file, group, key } => Ok(ini::get(&read_file(&env.sys(file.path()))?, group, key)),
            AdminTarget::Json { .. } => self.read(env),
            AdminTarget::Unit { unit } => Ok(system::unit_state(env, unit).map(|s| normalize_unit_state(&s).to_string())),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            AdminTarget::Ini { file, group, key } => format!("{} [{group}] {key}", file.path()),
            AdminTarget::Json { file, pointer } => format!("{} {}", file.path(), pointer.join("/")),
            AdminTarget::Unit { unit } => format!("systemd unit {unit}"),
        }
    }
}

/// When a system setting is shown at all.
#[derive(Clone, Debug)]
pub enum Condition {
    Always,
    UnitExists,
    FileExists(&'static str),
    Binary(&'static [&'static str]),
}

#[derive(Clone, Debug)]
pub struct AdminDef {
    pub id: String,
    pub app: &'static str,
    pub title: String,
    pub explanation: String,
    pub category: Category,
    pub section: Section,
    pub kind: Kind,
    pub target: AdminTarget,
    pub map: ValueMap,
    pub default: Option<Value>,
    pub recommended: Option<Value>,
    pub companions: Vec<&'static str>,
    pub condition: Condition,
}

/// Map every `systemctl is-enabled` answer to one the helper accepts, for undo.
pub fn normalize_unit_state(s: &str) -> &'static str {
    match s {
        "enabled" | "enabled-runtime" | "linked" | "linked-runtime" | "alias" => "enabled",
        "disabled" => "disabled",
        "masked" | "masked-runtime" => "masked",
        "indirect" => "indirect",
        _ => "static",
    }
}

/// Raw states a systemd unit may be set to through the helper.
pub const UNIT_RAWS: &[&str] = &["enabled", "disabled", "masked", "unmasked", "static", "indirect"];

fn opts(list: &[(&str, &str)]) -> Kind {
    Kind::Choice(
        list.iter()
            .map(|(id, label)| ChoiceOpt { id: id.to_string(), label: label.to_string() })
            .collect(),
    )
}

fn ch(s: &str) -> Value {
    Value::Choice(s.to_string())
}

fn choice_map(list: &[(&str, Option<&str>)]) -> ValueMap {
    ValueMap {
        entries: list.iter().map(|(id, raw)| (ch(id), raw.map(String::from))).collect(),
        aliases: vec![],
    }
}

#[allow(clippy::too_many_arguments)]
fn unit(
    id: &str,
    unit_name: &str,
    companions: &[&'static str],
    title: &str,
    explanation: &str,
    category: Category,
    section: Section,
    recommended: Option<bool>,
) -> AdminDef {
    AdminDef {
        id: format!("unit.{id}"),
        app: "system-services",
        title: title.into(),
        explanation: explanation.into(),
        category,
        section,
        kind: Kind::Bool,
        target: AdminTarget::Unit { unit: unit_name.into() },
        map: ValueMap::bool("enabled", "disabled")
            .alias("masked", Value::Bool(false))
            .alias("static", Value::Bool(true))
            .alias("indirect", Value::Bool(true))
            .alias("enabled-runtime", Value::Bool(true))
            .alias("linked", Value::Bool(true)),
        default: None,
        recommended: recommended.map(Value::Bool),
        companions: companions.to_vec(),
        condition: Condition::UnitExists,
    }
}

#[allow(clippy::too_many_arguments)]
fn ini_def(
    id: &str,
    app: &'static str,
    file: ManagedFile,
    group: &str,
    key: &str,
    title: &str,
    explanation: &str,
    category: Category,
    section: Section,
    kind: Kind,
    map: ValueMap,
    default: Option<Value>,
    recommended: Option<Value>,
    condition: Condition,
) -> AdminDef {
    AdminDef {
        id: id.into(),
        app,
        title: title.into(),
        explanation: explanation.into(),
        category,
        section,
        kind,
        target: AdminTarget::Ini { file, group: group.into(), key: key.into() },
        map,
        default,
        recommended,
        companions: vec![],
        condition,
    }
}

struct Policy {
    key: &'static [&'static str],
    title: &'static str,
    explanation: &'static str,
    category: Category,
    /// Raw JSON written when the switch is on. Off removes the policy.
    on: &'static str,
}

const CHROMIUM_POLICIES: &[Policy] = &[
    Policy { key: &["MetricsReportingEnabled"], title: "Block usage statistics and crash reports", explanation: "Stops the browser from sending usage statistics and crash reports to its maker, and stops anyone from turning it back on in the browser's own settings.", category: Category::Telemetry, on: "false" },
    Policy { key: &["UrlKeyedAnonymizedDataCollectionEnabled"], title: "Block sending the addresses of pages you visit", explanation: "\"Make searches and browsing better\" sends the address of every page you visit to the browser maker. This policy keeps it off.", category: Category::Tracking, on: "false" },
    Policy { key: &["SafeBrowsingExtendedReportingEnabled"], title: "Block extended Safe Browsing reports", explanation: "Extended reporting uploads information about pages and downloads to Google. Basic Safe Browsing protection still works.", category: Category::Telemetry, on: "false" },
    Policy { key: &["AlternateErrorPagesEnabled"], title: "Block online suggestions on error pages", explanation: "When a page cannot be found, the browser can send the address to a web service to suggest alternatives.", category: Category::Tracking, on: "false" },
    Policy { key: &["SpellCheckServiceEnabled"], title: "Block the online spell checker", explanation: "The enhanced spell checker sends what you type in text fields to the browser maker. Local spell checking still works.", category: Category::Telemetry, on: "false" },
    Policy { key: &["SearchSuggestEnabled"], title: "Turn off search suggestions", explanation: "Search suggestions send each letter you type in the address bar to your search engine.", category: Category::Tracking, on: "false" },
    Policy { key: &["NetworkPredictionOptions"], title: "Turn off preloading of pages", explanation: "Preloading connects to sites, and can load pages, that you have not opened yet.", category: Category::Network, on: "2" },
    Policy { key: &["WebRtcIPHandling"], title: "Hide your local IP address from WebRTC", explanation: "Stops video-call and peer-to-peer features from exposing your local network addresses to websites.", category: Category::Network, on: "\"default_public_interface_only\"" },
    Policy { key: &["BlockThirdPartyCookies"], title: "Block third-party cookies", explanation: "Stops other sites embedded in a page from setting cookies that follow you across the web.", category: Category::Tracking, on: "true" },
    Policy { key: &["BackgroundModeEnabled"], title: "Do not keep running in the background", explanation: "Stops the browser from running after you close its last window.", category: Category::Network, on: "false" },
    Policy { key: &["PrivacySandboxAdTopicsEnabled"], title: "Block ad topics (Privacy Sandbox)", explanation: "Ad topics let the browser tell sites which interests it has guessed from your browsing.", category: Category::Tracking, on: "false" },
    Policy { key: &["PrivacySandboxSiteEnabledAdsEnabled"], title: "Block site-suggested ads (Privacy Sandbox)", explanation: "Lets sites store interest groups in your browser for later ad auctions.", category: Category::Tracking, on: "false" },
    Policy { key: &["PrivacySandboxAdMeasurementEnabled"], title: "Block ad measurement (Privacy Sandbox)", explanation: "Lets sites and advertisers measure whether their ads led to visits or purchases.", category: Category::Tracking, on: "false" },
];

const BRAVE_POLICIES: &[Policy] = &[
    Policy { key: &["BraveP3AEnabled"], title: "Block Brave product analytics (P3A)", explanation: "P3A sends anonymous answers about how you use Brave.", category: Category::Telemetry, on: "false" },
    Policy { key: &["BraveStatsPingEnabled"], title: "Block the daily usage ping", explanation: "Brave sends a daily ping used to count active users.", category: Category::Telemetry, on: "false" },
    Policy { key: &["BraveWebDiscoveryEnabled"], title: "Block Web Discovery Project", explanation: "Web Discovery sends information about pages you visit to Brave Search.", category: Category::Tracking, on: "false" },
    Policy { key: &["BraveRewardsDisabled"], title: "Turn off Brave Rewards", explanation: "Removes Brave Rewards and its private ads.", category: Category::Tracking, on: "true" },
    Policy { key: &["BraveWalletDisabled"], title: "Turn off Brave Wallet", explanation: "Removes the built-in crypto wallet.", category: Category::Security, on: "true" },
    Policy { key: &["BraveVPNDisabled"], title: "Turn off Brave VPN", explanation: "Removes the Brave VPN button and offers.", category: Category::Network, on: "true" },
    Policy { key: &["BraveAIChatEnabled"], title: "Turn off Leo AI assistant", explanation: "Leo sends page content and your questions to Brave's AI service when you use it.", category: Category::Telemetry, on: "false" },
    Policy { key: &["BraveNewsDisabled"], title: "Turn off Brave News", explanation: "Removes the news feed on the new tab page.", category: Category::Tracking, on: "true" },
];

const FIREFOX_POLICIES: &[Policy] = &[
    Policy { key: &["policies", "DisableTelemetry"], title: "Block telemetry", explanation: "Turns off telemetry and health reports for every Firefox profile on this computer, and stops it being turned back on.", category: Category::Telemetry, on: "true" },
    Policy { key: &["policies", "DisableFirefoxStudies"], title: "Block Firefox studies", explanation: "Stops Mozilla from installing and running experiments in Firefox.", category: Category::Telemetry, on: "true" },
    Policy { key: &["policies", "DisablePocket"], title: "Turn off Pocket", explanation: "Removes the Pocket service and its stories.", category: Category::Tracking, on: "true" },
    Policy { key: &["policies", "FirefoxSuggest", "SponsoredSuggestions"], title: "Block sponsored suggestions in the address bar", explanation: "Firefox Suggest can show sponsored results based on what you type.", category: Category::Tracking, on: "false" },
    Policy { key: &["policies", "FirefoxSuggest", "ImproveSuggest"], title: "Block sending address bar input to improve Suggest", explanation: "Stops Firefox from sending what you type in the address bar to Mozilla to improve suggestions.", category: Category::Telemetry, on: "false" },
    Policy { key: &["policies", "NetworkPrediction"], title: "Turn off DNS prefetching and preconnecting", explanation: "Stops Firefox from looking up and connecting to links you have not clicked.", category: Category::Network, on: "false" },
    Policy { key: &["policies", "CaptivePortal"], title: "Turn off captive portal detection", explanation: "Firefox regularly contacts a Mozilla server to detect hotel and airport login pages. You may need to open those login pages yourself.", category: Category::Network, on: "false" },
    Policy { key: &["policies", "EnableTrackingProtection", "Value"], title: "Enforce tracking protection", explanation: "Turns on tracking protection in all windows, for every profile.", category: Category::Tracking, on: "true" },
];

fn policy_defs(
    out: &mut Vec<AdminDef>,
    app: &'static str,
    browser: &str,
    file: ManagedFile,
    list: &[Policy],
    binaries: &'static [&'static str],
) {
    for p in list {
        out.push(AdminDef {
            id: format!("policy.{app}.{}", p.key.join(".")),
            app,
            title: format!("{} (all users)", p.title),
            explanation: format!(
                "{} Enforced for every user through a {browser} policy ({}). Applies when {browser} restarts.",
                p.explanation,
                file.path()
            ),
            category: p.category,
            section: Section::Browsers,
            kind: Kind::Bool,
            target: AdminTarget::Json { file, pointer: p.key.iter().map(|s| s.to_string()).collect() },
            map: ValueMap::bool_or_absent(p.on),
            default: None,
            // Policies are optional hardening on top of the per-profile settings,
            // so they carry no recommendation of their own.
            recommended: None,
            companions: vec![],
            condition: Condition::Binary(binaries),
        });
    }
}

/// Every system-wide setting the program knows.
pub fn admin_defs() -> Vec<AdminDef> {
    use Category as C;
    use Section as S;
    let mut v = vec![
        unit("avahi", "avahi-daemon.service", &["avahi-daemon.socket"], "Announce this computer on the local network (Avahi)",
             "Avahi tells every device on your network this computer's name and services, and finds network printers and shares. Turning it off makes you less visible on public networks, but automatic printer discovery stops working.",
             C::Network, S::Network, Some(false)),
        unit("cups-browsed", "cups-browsed.service", &[], "Automatically add network printers (cups-browsed)",
             "Listens on the network for printers and adds them for you. It has had serious security holes and is rarely needed.",
             C::Network, S::Network, Some(false)),
        unit("bluetooth", "bluetooth.service", &[], "Bluetooth",
             "The Bluetooth service. Turn it off if you never use Bluetooth devices, so the computer cannot be seen or reached over Bluetooth.",
             C::Sharing, S::System, None),
        unit("sshd", "sshd.service", &[], "Remote login server (SSH)",
             "Lets other computers log in to this one over the network. Only needed if you connect to this computer from elsewhere.",
             C::Network, S::Network, None),
        unit("ssh", "ssh.service", &[], "Remote login server (SSH)",
             "Lets other computers log in to this one over the network. Only needed if you connect to this computer from elsewhere.",
             C::Network, S::Network, None),
        unit("modemmanager", "ModemManager.service", &[], "Mobile broadband modems (ModemManager)",
             "Manages 3G/4G/5G modems. Only needed if you have a mobile broadband modem.",
             C::Sharing, S::System, None),
        unit("pkgstats", "pkgstats.timer", &[], "Send package statistics (pkgstats)",
             "Sends the list of installed packages to the Arch Linux statistics service every week.",
             C::Telemetry, S::System, Some(false)),
        unit("apport", "apport.service", &[], "Crash reports (Apport)",
             "Collects crash reports and offers to send them to Ubuntu. Reports can contain memory from the crashed program.",
             C::Telemetry, S::System, Some(false)),
        unit("whoopsie", "whoopsie.service", &[], "Send crash reports (Whoopsie)",
             "Sends crash reports to Canonical's error tracker.",
             C::Telemetry, S::System, Some(false)),
        unit("abrtd", "abrtd.service", &[], "Crash reports (ABRT)",
             "Collects crash reports and can send them to Fedora or Red Hat.",
             C::Telemetry, S::System, Some(false)),
    ];
    // GeoClue is started on demand, so it is blocked by masking it.
    v.push(AdminDef {
        id: "unit.geoclue".into(),
        app: "system-services",
        title: "Location service (GeoClue)".into(),
        explanation: "GeoClue finds your location for apps that ask, often by sending nearby Wi-Fi networks to an online service. Turning it off blocks the service completely; apps can no longer get your location from it.".into(),
        category: C::Location,
        section: S::System,
        kind: Kind::Bool,
        target: AdminTarget::Unit { unit: "geoclue.service".into() },
        map: ValueMap::bool("unmasked", "masked")
            .alias("static", Value::Bool(true))
            .alias("enabled", Value::Bool(true))
            .alias("disabled", Value::Bool(true))
            .alias("indirect", Value::Bool(true)),
        default: None,
        recommended: Some(Value::Bool(false)),
        companions: vec![],
        condition: Condition::UnitExists,
    });

    let nm = Condition::Binary(&["NetworkManager"]);
    let mac_opts = [
        ("default", "NetworkManager default"),
        ("preserve", "Real address"),
        ("stable", "Random per network (stable)"),
        ("random", "New random address every time"),
    ];
    let mac_map = choice_map(&[
        ("default", None),
        ("preserve", Some("preserve")),
        ("permanent", Some("permanent")),
        ("stable", Some("stable")),
        ("random", Some("random")),
    ]);
    let mut wifi_mac_opts = mac_opts.to_vec();
    wifi_mac_opts.push(("permanent", "Factory address"));
    v.extend([
        ini_def("nm.wifi-mac", "networkmanager", ManagedFile::NetworkManager, "connection", "wifi.cloned-mac-address",
            "Wi-Fi hardware address (MAC)",
            "Your Wi-Fi card's hardware address is sent to every network you join and lets networks recognise you. \"Random per network\" uses a different address for each network but keeps it stable, so logins still work. Applies the next time you connect. A connection's own setting wins over this.",
            C::Tracking, S::Network, opts(&wifi_mac_opts), mac_map.clone(), Some(ch("default")), Some(ch("stable")), nm.clone()),
        ini_def("nm.eth-mac", "networkmanager", ManagedFile::NetworkManager, "connection", "ethernet.cloned-mac-address",
            "Wired hardware address (MAC)",
            "The same as above, for wired connections.",
            C::Tracking, S::Network, opts(&wifi_mac_opts), mac_map, Some(ch("default")), None, nm.clone()),
        ini_def("nm.scan-rand", "networkmanager", ManagedFile::NetworkManager, "device", "wifi.scan-rand-mac-address",
            "Random address while scanning for Wi-Fi",
            "Uses a random hardware address while looking for networks, so shops and public places cannot track you as you walk past.",
            C::Tracking, S::Network, Kind::Bool, ValueMap::bool("yes", "no").alias("true", Value::Bool(true)).alias("false", Value::Bool(false)), Some(Value::Bool(true)), Some(Value::Bool(true)), nm.clone()),
        ini_def("nm.connectivity", "networkmanager", ManagedFile::NetworkManager, "connectivity", "enabled",
            "Connectivity check",
            "NetworkManager regularly contacts a server run by your distribution to check that the internet works. The server sees your IP address and when you are online. Without it, login pages on public Wi-Fi are not detected automatically.",
            C::Network, S::Network, Kind::Bool, ValueMap::bool("true", "false"), Some(Value::Bool(true)), Some(Value::Bool(false)), nm.clone()),
        ini_def("nm.dhcp4-hostname", "networkmanager", ManagedFile::NetworkManager, "connection", "ipv4.dhcp-send-hostname",
            "Send this computer's name to networks (IPv4)",
            "When joining a network, the computer's name is sent to the router. On public networks this tells others who you are, especially if the name includes your own name.",
            C::Network, S::Network, Kind::Bool, ValueMap::bool("true", "false"), Some(Value::Bool(true)), Some(Value::Bool(false)), nm.clone()),
        ini_def("nm.dhcp6-hostname", "networkmanager", ManagedFile::NetworkManager, "connection", "ipv6.dhcp-send-hostname",
            "Send this computer's name to networks (IPv6)",
            "The same as above, for IPv6.",
            C::Network, S::Network, Kind::Bool, ValueMap::bool("true", "false"), Some(Value::Bool(true)), Some(Value::Bool(false)), nm.clone()),
        ini_def("nm.ip6-privacy", "networkmanager", ManagedFile::NetworkManager, "connection", "ipv6.ip6-privacy",
            "IPv6 privacy addresses",
            "Without privacy addresses, your IPv6 address can be built from your hardware address and stays the same on every network. Temporary addresses change regularly.",
            C::Tracking, S::Network,
            opts(&[("default", "NetworkManager default"), ("0", "Off"), ("1", "On, but prefer the fixed address"), ("2", "On, prefer temporary addresses")]),
            choice_map(&[("default", None), ("0", Some("0")), ("1", Some("1")), ("2", Some("2"))]),
            Some(ch("default")), Some(ch("2")), nm),
    ]);

    let systemd = Condition::Binary(&["systemctl"]);
    v.extend([
        ini_def("journald.storage", "system-logs", ManagedFile::Journald, "Journal", "Storage",
            "Where system logs are kept",
            "The system log records what programs do, including names of files, devices and networks. \"Only in memory\" forgets everything at shutdown, but makes problems harder to investigate afterwards. Applies after a restart.",
            C::History, S::System,
            opts(&[("default", "System default"), ("auto", "On disk if /var/log/journal exists"), ("persistent", "On disk"), ("volatile", "Only in memory, gone at shutdown")]),
            choice_map(&[("default", None), ("auto", Some("auto")), ("persistent", Some("persistent")), ("volatile", Some("volatile"))]),
            Some(ch("default")), None, systemd.clone()),
        ini_def("journald.retention", "system-logs", ManagedFile::Journald, "Journal", "MaxRetentionSec",
            "How long system logs are kept",
            "Older log entries are deleted automatically. Applies after a restart.",
            C::History, S::System,
            opts(&[("default", "Until the size limit"), ("1week", "1 week"), ("1month", "1 month"), ("3month", "3 months")]),
            choice_map(&[("default", None), ("1week", Some("1week")), ("1month", Some("1month")), ("3month", Some("3month"))]),
            Some(ch("default")), None, systemd.clone()),
        ini_def("coredump.storage", "system-logs", ManagedFile::Coredump, "Coredump", "Storage",
            "Keep memory dumps of crashed programs",
            "When a program crashes, a copy of its memory is saved. It can contain passwords, open documents and messages. Developers use these dumps to fix bugs.",
            C::History, S::System,
            opts(&[("default", "System default (keep on disk)"), ("external", "Keep on disk"), ("journal", "Keep in the system log"), ("none", "Do not keep")]),
            choice_map(&[("default", None), ("external", Some("external")), ("journal", Some("journal")), ("none", Some("none"))]),
            Some(ch("default")), Some(ch("none")), systemd),
        ini_def("popcon", "distro-telemetry", ManagedFile::Popcon, "", "PARTICIPATE",
            "Popularity contest",
            "Sends the list of installed packages and how often they are used to Debian or Ubuntu every week.",
            C::Telemetry, S::System, Kind::Bool, ValueMap::bool("\"yes\"", "\"no\""), Some(Value::Bool(false)), Some(Value::Bool(false)),
            Condition::FileExists("/etc/popularity-contest.conf")),
        ini_def("motd-news", "distro-telemetry", ManagedFile::MotdNews, "", "ENABLED",
            "Message-of-the-day news",
            "Ubuntu downloads news for the login message, sending your Ubuntu version, hardware and uptime.",
            C::Telemetry, S::System, Kind::Bool, ValueMap::bool("1", "0"), Some(Value::Bool(true)), Some(Value::Bool(false)),
            Condition::FileExists("/etc/default/motd-news")),
        ini_def("apport.enabled", "distro-telemetry", ManagedFile::Apport, "", "enabled",
            "Crash report collection (Apport)",
            "Collects crash reports and asks to send them to Ubuntu.",
            C::Telemetry, S::System, Kind::Bool, ValueMap::bool("1", "0"), Some(Value::Bool(true)), Some(Value::Bool(false)),
            Condition::FileExists("/etc/default/apport")),
        ini_def("whoopsie.report", "distro-telemetry", ManagedFile::Whoopsie, "General", "report_crashes",
            "Send crash reports to Canonical",
            "Whoopsie uploads crash reports to Canonical's error tracker.",
            C::Telemetry, S::System, Kind::Bool, ValueMap::bool("true", "false"), Some(Value::Bool(true)), Some(Value::Bool(false)),
            Condition::FileExists("/etc/whoopsie")),
    ]);

    policy_defs(&mut v, "firefox", "Firefox", ManagedFile::FirefoxPolicies, FIREFOX_POLICIES, &["firefox", "/usr/lib/firefox/firefox"]);
    for (app, name, file, bins) in [
        ("brave", "Brave", ManagedFile::BravePolicies, &["brave", "brave-browser", "/opt/brave-bin/brave", "/opt/brave.com/brave/brave"] as &'static [&'static str]),
        ("google-chrome", "Chrome", ManagedFile::ChromePolicies, &["google-chrome-stable", "google-chrome"]),
        ("chromium", "Chromium", ManagedFile::ChromiumPolicies, &["chromium", "chromium-browser"]),
        ("microsoft-edge", "Edge", ManagedFile::EdgePolicies, &["microsoft-edge-stable", "microsoft-edge"]),
    ] {
        policy_defs(&mut v, app, name, file, CHROMIUM_POLICIES, bins);
        if app == "brave" {
            policy_defs(&mut v, app, name, file, BRAVE_POLICIES, bins);
        }
    }
    v
}

/// One requested change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdminOp {
    pub target: AdminTarget,
    pub raw: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminRequest {
    pub ops: Vec<AdminOp>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminResponse {
    pub results: Vec<OpResult>,
}

/// Check a request against the table. Returns the matching definition.
pub fn validate(op: &AdminOp, defs: &[AdminDef]) -> Result<AdminDef, String> {
    let def = defs
        .iter()
        .find(|d| d.target == op.target)
        .ok_or_else(|| format!("not an allowed setting: {}", op.target.describe()))?;
    let ok = match &op.target {
        AdminTarget::Unit { .. } => op.raw.as_deref().is_some_and(|r| {
            UNIT_RAWS.contains(&r)
                && (def.map.raws().contains(&Some(r.to_string()))
                    || def.map.aliases.iter().any(|(a, _)| a == r)
                    || matches!(r, "unmasked" | "static" | "indirect"))
        }),
        _ => {
            def.map.raws().contains(&op.raw)
                || op.raw.as_ref().is_some_and(|r| def.map.aliases.iter().any(|(a, _)| a == r))
                || (op.raw.is_none() && !def.target_file_is_distro())
        }
    };
    if ok {
        Ok(def.clone())
    } else {
        Err(format!("value {:?} is not allowed for {}", op.raw, op.target.describe()))
    }
}

impl AdminDef {
    fn target_file_is_distro(&self) -> bool {
        match &self.target {
            AdminTarget::Ini { file, .. } | AdminTarget::Json { file, .. } => file.must_exist(),
            AdminTarget::Unit { .. } => false,
        }
    }
}

fn systemctl(env: &Env, args: &[&str], dry: bool, log: &mut Vec<String>) -> Result<(), String> {
    log.push(format!("systemctl {}", args.join(" ")));
    if dry || !env.live {
        return Ok(());
    }
    let bin = ["/usr/bin/systemctl", "/bin/systemctl"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or("systemctl not found")?;
    let out = Command::new(bin).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Carry out one validated change. Used by the root helper and by tests (with a fake root).
pub fn execute(env: &Env, op: &AdminOp, def: &AdminDef, dry: bool) -> Result<String, String> {
    let mut log = Vec::new();
    match &op.target {
        AdminTarget::Unit { unit } => {
            let raw = op.raw.as_deref().unwrap_or("");
            let mut units: Vec<&str> = vec![unit.as_str()];
            units.extend(def.companions.iter().copied());
            let current = system::unit_state(env, unit).unwrap_or_default();
            if current == "masked" || matches!(raw, "unmasked" | "static" | "indirect") {
                let mut a = vec!["unmask"];
                a.extend(&units);
                systemctl(env, &a, dry, &mut log)?;
            }
            let verb: Option<&[&str]> = match raw {
                "enabled" => Some(&["enable", "--now"]),
                "disabled" => Some(&["disable", "--now"]),
                "masked" => Some(&["mask", "--now"]),
                _ => None,
            };
            if let Some(v) = verb {
                let mut a: Vec<&str> = v.to_vec();
                a.extend(&units);
                systemctl(env, &a, dry, &mut log)?;
            }
        }
        AdminTarget::Ini { file, group, key } => {
            let path = env.sys(file.path());
            check_path(&path, file.must_exist())?;
            let old = read_file(&path)?;
            let new = ini::set(&old, group, key, op.raw.as_deref());
            log.push(format!("write {} [{group}] {key} = {:?}", file.path(), op.raw));
            if !dry && new != old {
                write_file(&path, &new)?;
            }
            if let Some(a) = file.post_action() {
                let _ = systemctl(env, a, dry, &mut log);
            }
        }
        AdminTarget::Json { file, pointer } => {
            let path = env.sys(file.path());
            check_path(&path, file.must_exist())?;
            let old = read_file(&path)?;
            let new = jsonc::set(&old, pointer, op.raw.as_deref())?;
            log.push(format!("write {} {} = {:?}", file.path(), pointer.join("/"), op.raw));
            if !dry && new != old {
                write_file(&path, &new)?;
            }
        }
    }
    Ok(log.join("; "))
}

fn check_path(path: &std::path::Path, must_exist: bool) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(format!("{} is a symbolic link; refusing to write", path.display())),
        Ok(_) => Ok(()),
        Err(_) if must_exist => Err(format!("{} does not exist", path.display())),
        Err(_) => Ok(()),
    }
}

/// Validate and run a whole request.
pub fn handle_request(env: &Env, req: &AdminRequest) -> AdminResponse {
    let defs = admin_defs();
    let results = req
        .ops
        .iter()
        .map(|op| match validate(op, &defs).and_then(|d| execute(env, op, &d, req.dry_run)) {
            Ok(m) => OpResult { ok: true, message: m },
            Err(e) => OpResult { ok: false, message: e },
        })
        .collect();
    AdminResponse { results }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_env(name: &str) -> Env {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/test-admin").join(name);
        let _ = std::fs::remove_dir_all(&base);
        Env::for_test(&base)
    }

    #[test]
    fn ids_are_unique() {
        let defs = admin_defs();
        let mut ids: Vec<_> = defs.iter().map(|d| d.id.clone()).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len());
    }

    #[test]
    fn rejects_unknown_targets_and_values() {
        let defs = admin_defs();
        let bad_unit = AdminOp { target: AdminTarget::Unit { unit: "sshd-evil.service".into() }, raw: Some("disabled".into()) };
        assert!(validate(&bad_unit, &defs).is_err());
        let bad_val = AdminOp { target: AdminTarget::Unit { unit: "avahi-daemon.service".into() }, raw: Some("rm -rf".into()) };
        assert!(validate(&bad_val, &defs).is_err());
        let bad_key = AdminOp {
            target: AdminTarget::Ini { file: ManagedFile::NetworkManager, group: "main".into(), key: "plugins".into() },
            raw: Some("keyfile".into()),
        };
        assert!(validate(&bad_key, &defs).is_err());
        let bad_ini_val = AdminOp {
            target: AdminTarget::Ini { file: ManagedFile::NetworkManager, group: "connectivity".into(), key: "enabled".into() },
            raw: Some("true\n[main]\nplugins=evil".into()),
        };
        assert!(validate(&bad_ini_val, &defs).is_err());
        let bad_policy = AdminOp {
            target: AdminTarget::Json { file: ManagedFile::BravePolicies, pointer: vec!["ExtensionInstallForcelist".into()] },
            raw: Some("[\"x\"]".into()),
        };
        assert!(validate(&bad_policy, &defs).is_err());
        let distro_remove = AdminOp {
            target: AdminTarget::Ini { file: ManagedFile::Popcon, group: "".into(), key: "PARTICIPATE".into() },
            raw: None,
        };
        assert!(validate(&distro_remove, &defs).is_err());
    }

    #[test]
    fn executes_ini_and_json_under_fake_root() {
        let env = test_env("exec");
        let req = AdminRequest {
            ops: vec![
                AdminOp {
                    target: AdminTarget::Ini { file: ManagedFile::NetworkManager, group: "connectivity".into(), key: "enabled".into() },
                    raw: Some("false".into()),
                },
                AdminOp {
                    target: AdminTarget::Json { file: ManagedFile::BravePolicies, pointer: vec!["BraveP3AEnabled".into()] },
                    raw: Some("false".into()),
                },
                AdminOp {
                    target: AdminTarget::Json { file: ManagedFile::FirefoxPolicies, pointer: vec!["policies".into(), "DisableTelemetry".into()] },
                    raw: Some("true".into()),
                },
                AdminOp {
                    target: AdminTarget::Ini { file: ManagedFile::Popcon, group: "".into(), key: "PARTICIPATE".into() },
                    raw: Some("\"no\"".into()),
                },
            ],
            dry_run: false,
        };
        let resp = handle_request(&env, &req);
        assert!(resp.results[0].ok, "{:?}", resp.results[0]);
        assert!(resp.results[1].ok);
        assert!(resp.results[2].ok);
        assert!(!resp.results[3].ok, "popcon file does not exist, must not be created");
        let nm = std::fs::read_to_string(env.sys(ManagedFile::NetworkManager.path())).unwrap();
        assert_eq!(nm, "[connectivity]\nenabled=false\n");
        let t = AdminTarget::Json { file: ManagedFile::FirefoxPolicies, pointer: vec!["policies".into(), "DisableTelemetry".into()] };
        assert_eq!(t.read(&env).unwrap().as_deref(), Some("true"));
        // Removing again.
        let rm = AdminRequest {
            ops: vec![AdminOp {
                target: AdminTarget::Json { file: ManagedFile::BravePolicies, pointer: vec!["BraveP3AEnabled".into()] },
                raw: None,
            }],
            dry_run: false,
        };
        assert!(handle_request(&env, &rm).results[0].ok);
    }

    #[test]
    fn undo_snapshot_ignores_other_files() {
        use crate::target::Target;
        let env = test_env("own");
        crate::target::write_file(&env.sys("/etc/systemd/journald.conf"), "[Journal]\nMaxRetentionSec=2week\n").unwrap();
        let t = AdminTarget::Ini { file: ManagedFile::Journald, group: "Journal".into(), key: "MaxRetentionSec".into() };
        // Shown value comes from the distribution file...
        assert_eq!(t.read(&env).unwrap().as_deref(), Some("2week"));
        // ...but the undo snapshot is our own (absent) key, which the helper accepts.
        let snap = Target::Admin(t.clone()).snapshot(&env).unwrap();
        assert_eq!(snap, crate::target::Snapshot::Raw(None));
        assert!(validate(&AdminOp { target: t.clone(), raw: None }, &admin_defs()).is_ok());
        // A later drop-in overrides ours.
        let req = AdminRequest { ops: vec![AdminOp { target: t.clone(), raw: Some("1month".into()) }], dry_run: false };
        assert!(handle_request(&env, &req).results[0].ok);
        assert_eq!(t.read(&env).unwrap().as_deref(), Some("1month"));
        crate::target::write_file(&env.sys("/etc/systemd/journald.conf.d/99-local.conf"), "[Journal]\nMaxRetentionSec=1week\n").unwrap();
        assert_eq!(t.read(&env).unwrap().as_deref(), Some("1week"));
    }

    #[test]
    fn unit_states_normalise() {
        assert_eq!(normalize_unit_state("enabled-runtime"), "enabled");
        assert_eq!(normalize_unit_state("generated"), "static");
        assert_eq!(normalize_unit_state("masked-runtime"), "masked");
    }

    #[test]
    fn dry_run_writes_nothing() {
        let env = test_env("dry");
        let req = AdminRequest {
            ops: vec![AdminOp {
                target: AdminTarget::Ini { file: ManagedFile::Coredump, group: "Coredump".into(), key: "Storage".into() },
                raw: Some("none".into()),
            }],
            dry_run: true,
        };
        let r = handle_request(&env, &req);
        assert!(r.results[0].ok);
        assert!(!env.sys(ManagedFile::Coredump.path()).exists());
    }
}
