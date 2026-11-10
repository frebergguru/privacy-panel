//! Firefox-family and Chromium-family browsers, per profile.

use std::path::{Path, PathBuf};

use super::{setting, Collector};
use crate::backends::{ini, jsonc};
use crate::model::{Category as C, Section as S, Setting, ValueMap};
use crate::target::{read_file, Target};

pub fn collect(c: &mut Collector) {
    for fam in MOZ_FAMILIES {
        mozilla(c, fam);
    }
    for fam in CHROMIUM_FAMILIES {
        chromium(c, fam);
    }
}

// ---------------------------------------------------------------- Mozilla

struct MozFamily {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    dirs: &'static [(&'static str, &'static str)],
    desktop: &'static [&'static str],
    processes: &'static [&'static str],
    mail: bool,
}

const MOZ_FAMILIES: &[MozFamily] = &[
    MozFamily { id: "firefox", name: "Firefox", icon: "firefox",
        dirs: &[("$XDG_CONFIG_HOME/mozilla/firefox", ""), ("~/.mozilla/firefox", ""), ("~/.var/app/org.mozilla.firefox/.mozilla/firefox", "Flatpak"), ("~/snap/firefox/common/.mozilla/firefox", "Snap")],
        desktop: &["firefox", "org.mozilla.firefox", "firefox-esr", "firefox_firefox"], processes: &["firefox", "firefox-bin", "firefox-esr"], mail: false },
    MozFamily { id: "librewolf", name: "LibreWolf", icon: "librewolf",
        dirs: &[("~/.librewolf", ""), ("~/.var/app/io.gitlab.librewolf-community/.librewolf", "Flatpak")],
        desktop: &["librewolf", "io.gitlab.librewolf-community"], processes: &["librewolf", "librewolf-bin"], mail: false },
    MozFamily { id: "floorp", name: "Floorp", icon: "floorp",
        dirs: &[("~/.floorp", ""), ("~/.var/app/one.ablaze.floorp/.floorp", "Flatpak")],
        desktop: &["floorp", "one.ablaze.floorp"], processes: &["floorp", "floorp-bin"], mail: false },
    MozFamily { id: "waterfox", name: "Waterfox", icon: "waterfox",
        dirs: &[("~/.waterfox", ""), ("~/.var/app/net.waterfox.waterfox/.waterfox", "Flatpak")],
        desktop: &["waterfox", "net.waterfox.waterfox"], processes: &["waterfox", "waterfox-bin"], mail: false },
    MozFamily { id: "zen", name: "Zen Browser", icon: "zen-browser",
        dirs: &[("~/.zen", ""), ("~/.var/app/app.zen_browser.zen/.zen", "Flatpak")],
        desktop: &["zen", "zen-browser", "app.zen_browser.zen"], processes: &["zen", "zen-bin"], mail: false },
    MozFamily { id: "thunderbird", name: "Thunderbird", icon: "thunderbird",
        dirs: &[("$XDG_CONFIG_HOME/thunderbird", ""), ("~/.thunderbird", ""), ("~/.var/app/org.mozilla.Thunderbird/.thunderbird", "Flatpak"), ("~/snap/thunderbird/common/.thunderbird", "Snap")],
        desktop: &["thunderbird", "org.mozilla.Thunderbird", "net.thunderbird.Thunderbird"], processes: &["thunderbird", "thunderbird-bin"], mail: true },
];

/// (pref(s), title, explanation, category, default, recommended). Booleans.
type MozBool = (&'static [&'static str], &'static str, &'static str, C, bool, Option<bool>);

const MOZ_BROWSER_PREFS: &[MozBool] = &[
    (&["datareporting.healthreport.uploadEnabled"], "Send technical and interaction data to Mozilla",
     "Firefox regularly sends data about its performance, hardware and how you use its features.", C::Telemetry, true, Some(false)),
    (&["app.shield.optoutstudies.enabled"], "Allow Firefox to install and run studies",
     "Mozilla can try out features and changes on your copy of Firefox.", C::Telemetry, true, Some(false)),
    (&["browser.discovery.enabled"], "Personalised extension recommendations",
     "Uses data about your Firefox to recommend extensions.", C::Telemetry, true, Some(false)),
    (&["browser.crashReports.unsubmittedCheck.autoSubmit2"], "Send crash reports automatically",
     "Crash reports can contain the address of the page you were on and parts of memory.", C::Telemetry, false, Some(false)),
    (&["browser.newtabpage.activity-stream.feeds.telemetry", "browser.newtabpage.activity-stream.telemetry"], "New tab page telemetry",
     "Sends information about how you use the new tab page.", C::Telemetry, true, Some(false)),
    (&["browser.newtabpage.activity-stream.showSponsored"], "Sponsored stories on the new tab page",
     "Shows paid content chosen for you on the new tab page.", C::Tracking, true, Some(false)),
    (&["browser.newtabpage.activity-stream.showSponsoredTopSites"], "Sponsored shortcuts on the new tab page",
     "Shows paid links among your shortcuts.", C::Tracking, true, Some(false)),
    (&["browser.urlbar.suggest.quicksuggest.sponsored"], "Sponsored suggestions in the address bar",
     "Firefox Suggest can show paid results based on what you type.", C::Tracking, true, Some(false)),
    (&["browser.urlbar.suggest.quicksuggest.nonsponsored"], "Suggestions from the web in the address bar",
     "Firefox Suggest shows results from Mozilla's partners based on what you type.", C::Tracking, true, Some(false)),
    (&["extensions.pocket.enabled"], "Pocket",
     "Mozilla's save-for-later service, built into the browser.", C::Tracking, true, Some(false)),
    (&["dom.private-attribution.submission.enabled"], "Website advertising measurement",
     "Lets sites ask Firefox to report, anonymously, whether their ads led to a visit or purchase.", C::Tracking, true, Some(false)),
    (&["privacy.trackingprotection.enabled"], "Tracking protection in all windows",
     "Blocks known trackers in normal windows too, not only in private windows.", C::Tracking, false, Some(true)),
    (&["privacy.globalprivacycontrol.enabled"], "Tell websites not to sell or share your data",
     "Sends the Global Privacy Control signal to every site.", C::Tracking, false, Some(true)),
    (&["dom.security.https_only_mode"], "HTTPS-only mode",
     "Only connects to sites over encrypted connections, and warns before opening an unencrypted one.", C::Security, false, Some(true)),
    (&["media.peerconnection.ice.default_address_only"], "Hide your local IP address from WebRTC",
     "Stops video-call features from exposing your local network addresses to websites. Calls keep working.", C::Network, false, Some(true)),
    (&["network.prefetch-next"], "Preload linked pages",
     "Loads pages that a site says you may open next, before you click.", C::Network, true, Some(false)),
    (&["network.dns.disablePrefetch"], "Do not look up links in advance",
     "Normally Firefox looks up the addresses of links on a page before you click them.", C::Network, false, Some(true)),
    (&["browser.safebrowsing.downloads.remote.enabled"], "Check downloads with Google",
     "Sends information about some downloaded programs to Google to check them. Local checks of known bad sites keep working when this is off.", C::Telemetry, true, Some(false)),
    (&["network.captive-portal-service.enabled"], "Detect login pages on public Wi-Fi",
     "Firefox regularly contacts a Mozilla server to find hotel and airport login pages.", C::Network, true, None),
    (&["network.connectivity-service.enabled"], "Check internet connectivity",
     "Firefox regularly contacts a Mozilla server to check whether IPv4 and IPv6 work.", C::Network, true, Some(false)),
    (&["browser.search.suggest.enabled"], "Search suggestions",
     "Sends what you type in the address bar to your search engine as you type.", C::Tracking, true, None),
    (&["geo.enabled"], "Let websites ask for your location",
     "When on, sites can ask for your location, and Firefox asks you each time. When off, they cannot ask at all.", C::Location, true, None),
    (&["browser.ml.chat.enabled"], "AI chatbot in the sidebar",
     "Lets you send page content to an AI chatbot of your choice.", C::Telemetry, true, None),
];

const MOZ_MAIL_PREFS: &[MozBool] = &[
    (&["datareporting.healthreport.uploadEnabled"], "Send technical and interaction data",
     "Thunderbird regularly sends data about its performance and how you use it.", C::Telemetry, true, Some(false)),
    (&["mailnews.message_display.disable_remote_image"], "Block remote content in messages",
     "Images loaded from the internet tell the sender when and where you opened their email.", C::Tracking, true, Some(true)),
    (&["mailnews.start_page.enabled"], "Show a web page when Thunderbird starts",
     "Loads a start page from the internet each time Thunderbird opens.", C::Network, true, Some(false)),
    (&["mailnews.headers.sendUserAgent"], "Tell recipients which mail program you use",
     "Adds your Thunderbird version and operating system to every message you send.", C::Tracking, false, Some(false)),
    (&["privacy.donottrackheader.enabled"], "Ask websites not to track you",
     "Sends the Do Not Track signal when Thunderbird loads web content.", C::Tracking, false, Some(true)),
];

fn mozilla_profiles(dir: &Path) -> Vec<(String, PathBuf)> {
    let text = read_file(&dir.join("profiles.ini")).unwrap_or_default();
    let mut out = Vec::new();
    for g in ini::groups(&text) {
        if !g.starts_with("Profile") {
            continue;
        }
        let Some(path) = ini::get(&text, &g, "Path") else { continue };
        let rel = ini::get(&text, &g, "IsRelative").as_deref() != Some("0");
        let p = if rel { dir.join(&path) } else { PathBuf::from(&path) };
        if p.join("prefs.js").exists() || p.join("times.json").exists() {
            let name = ini::get(&text, &g, "Name").unwrap_or(path);
            out.push((name, p));
        }
    }
    out
}

fn mozilla(c: &mut Collector, fam: &MozFamily) {
    let mut profiles: Vec<(String, PathBuf)> = Vec::new();
    for (d, label) in fam.dirs {
        for (name, p) in mozilla_profiles(&c.env.expand(d)) {
            let label = if label.is_empty() { format!("Profile: {name}") } else { format!("Profile: {name} ({label})") };
            profiles.push((label, p));
        }
    }
    let installed = fam.desktop.iter().any(|d| c.inv.has_desktop(d));
    if profiles.is_empty() && !installed {
        return;
    }
    let mut app = c.known_app(fam.id, fam.name, fam.icon, fam.desktop, fam.processes);
    if app.desktop_ids.iter().any(|d| d.contains('.')) && app.desktop_ids.len() == 1 {
        app.source = crate::model::AppSource::Flatpak;
    }
    c.app(app);
    let procs: Vec<String> = fam.processes.iter().map(|s| s.to_string()).collect();
    if profiles.is_empty() {
        c.add(setting(format!("{}.noprofile", fam.id), fam.id, S::Browsers, C::Telemetry, "No profile yet",
            format!("{} has not been started yet, so there are no profile settings to show.", fam.name)).info("Start it once, then refresh"));
        return;
    }
    let prefs = if fam.mail { MOZ_MAIL_PREFS } else { MOZ_BROWSER_PREFS };
    let section = if fam.mail { S::Applications } else { S::Browsers };
    for (label, dir) in &profiles {
        let pid = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for (names, title, expl, cat, def, rec) in prefs {
            let targets: Vec<Target> = names
                .iter()
                .map(|n| Target::MozPref { profile: dir.clone(), pref: n.to_string() })
                .collect();
            let target = if targets.len() == 1 { targets.into_iter().next().unwrap() } else { Target::Multi(targets) };
            c.add(setting(format!("{}@{pid}.{}", fam.id, names[0]), fam.id, section, *cat, *title,
                format!("{expl} Applies the next time {} starts.", fam.name))
                .boolean(target, ValueMap::bool("true", "false"), Some(*def), *rec)
                .group(label.clone())
                .closed(&procs));
        }
        if !fam.mail {
            c.add(setting(format!("{}@{pid}.trr", fam.id), fam.id, section, C::Network, "DNS over HTTPS",
                format!("Encrypts the look-ups of site addresses so your network cannot see or change them. They go to the chosen DNS provider instead. Applies the next time {} starts.", fam.name))
                .choice(Target::MozPref { profile: dir.clone(), pref: "network.trr.mode".into() }, &[
                    ("0", "Default", Some("0")),
                    ("2", "Increased protection (fall back to normal DNS)", Some("2")),
                    ("3", "Max protection (DNS over HTTPS only)", Some("3")),
                    ("5", "Off", Some("5")),
                ], Some("0"), None)
                .group(label.clone())
                .closed(&procs));
        }
    }
}

// ---------------------------------------------------------------- Chromium

struct ChromiumFamily {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    dirs: &'static [(&'static str, &'static str)],
    desktop: &'static [&'static str],
    processes: &'static [&'static str],
    brave: bool,
}

const CHROMIUM_FAMILIES: &[ChromiumFamily] = &[
    ChromiumFamily { id: "brave", name: "Brave", icon: "brave-browser",
        dirs: &[("$XDG_CONFIG_HOME/BraveSoftware/Brave-Browser", ""), ("$XDG_CONFIG_HOME/BraveSoftware/Brave-Browser-Beta", "Beta"), ("$XDG_CONFIG_HOME/BraveSoftware/Brave-Browser-Nightly", "Nightly"), ("~/.var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser", "Flatpak")],
        desktop: &["brave-browser", "com.brave.Browser", "brave-browser-beta", "brave-browser-nightly", "brave"], processes: &["brave", "brave-browser"], brave: true },
    ChromiumFamily { id: "google-chrome", name: "Google Chrome", icon: "google-chrome",
        dirs: &[("$XDG_CONFIG_HOME/google-chrome", ""), ("$XDG_CONFIG_HOME/google-chrome-beta", "Beta"), ("~/.var/app/com.google.Chrome/config/google-chrome", "Flatpak")],
        desktop: &["google-chrome", "com.google.Chrome", "google-chrome-beta"], processes: &["chrome"], brave: false },
    ChromiumFamily { id: "chromium", name: "Chromium", icon: "chromium",
        dirs: &[("$XDG_CONFIG_HOME/chromium", ""), ("~/.var/app/org.chromium.Chromium/config/chromium", "Flatpak"), ("~/snap/chromium/common/chromium", "Snap")],
        desktop: &["chromium", "org.chromium.Chromium", "chromium-browser", "chromium_chromium"], processes: &["chromium", "chromium-browser"], brave: false },
    ChromiumFamily { id: "vivaldi", name: "Vivaldi", icon: "vivaldi",
        dirs: &[("$XDG_CONFIG_HOME/vivaldi", ""), ("~/.var/app/com.vivaldi.Vivaldi/config/vivaldi", "Flatpak")],
        desktop: &["vivaldi-stable", "com.vivaldi.Vivaldi", "vivaldi"], processes: &["vivaldi-bin", "vivaldi"], brave: false },
    ChromiumFamily { id: "microsoft-edge", name: "Microsoft Edge", icon: "microsoft-edge",
        dirs: &[("$XDG_CONFIG_HOME/microsoft-edge", ""), ("~/.var/app/com.microsoft.Edge/config/microsoft-edge", "Flatpak")],
        desktop: &["microsoft-edge", "com.microsoft.Edge"], processes: &["msedge"], brave: false },
    ChromiumFamily { id: "opera", name: "Opera", icon: "opera",
        dirs: &[("$XDG_CONFIG_HOME/opera", ""), ("~/.var/app/com.opera.Opera/config/opera", "Flatpak")],
        desktop: &["opera", "com.opera.Opera"], processes: &["opera"], brave: false },
];

fn p(s: &str) -> Vec<String> {
    s.split('.').map(String::from).collect()
}

fn chromium_profiles(dir: &Path) -> Vec<(String, PathBuf)> {
    let local = read_file(&dir.join("Local State")).unwrap_or_default();
    let info = jsonc::to_value(&local).ok();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().join("Preferences").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for n in names {
        let shown = info
            .as_ref()
            .and_then(|v| v.pointer(&format!("/profile/info_cache/{}/name", n.replace('~', "~0").replace('/', "~1"))))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| n.clone());
        out.push((shown, dir.join(&n)));
    }
    out
}

fn chromium(c: &mut Collector, fam: &ChromiumFamily) {
    let installed = fam.desktop.iter().any(|d| c.inv.has_desktop(d));
    if !installed {
        return;
    }
    let app = c.known_app(fam.id, fam.name, fam.icon, fam.desktop, fam.processes);
    c.app(app);
    let procs: Vec<String> = fam.processes.iter().map(|s| s.to_string()).collect();
    let tf = || ValueMap::bool("true", "false");
    let mut any = false;
    for (d, label) in fam.dirs {
        let dir = c.env.expand(d);
        if !dir.join("Local State").exists() {
            continue;
        }
        any = true;
        let tag = if label.is_empty() { String::new() } else { format!(" ({label})") };
        let ls = dir.join("Local State");
        let js = |pointer: &str, path: &Path| Target::Json { path: path.to_path_buf(), pointer: p(pointer) };
        let browser_group = format!("Whole browser{tag}");
        let note = format!("Applies the next time {} starts.", fam.name);
        let mut list: Vec<Setting> = vec![
            setting(format!("{}{tag}.metrics", fam.id), fam.id, S::Browsers, C::Telemetry, "Send usage statistics and crash reports",
                format!("Sends statistics about how you use the browser and reports when it crashes. {note}"))
                .boolean(js("user_experience_metrics.reporting_enabled", &ls), tf(), Some(false), Some(false)),
            setting(format!("{}{tag}.background", fam.id), fam.id, S::Browsers, C::Network, "Keep running after the last window closes",
                format!("Lets extensions and apps keep running in the background. {note}"))
                .boolean(js("background_mode.enabled", &ls), tf(), Some(true), Some(false)),
        ];
        if fam.brave {
            list.push(setting(format!("{}{tag}.p3a", fam.id), fam.id, S::Browsers, C::Telemetry, "Privacy-preserving product analytics (P3A)",
                format!("Sends anonymous answers about how you use Brave. {note}"))
                .boolean(js("brave.p3a.enabled", &ls), tf(), Some(true), Some(false)));
            list.push(setting(format!("{}{tag}.stats", fam.id), fam.id, S::Browsers, C::Telemetry, "Daily usage ping",
                format!("Sends a daily ping that Brave uses to count active users. {note}"))
                .boolean(js("brave.stats.reporting_enabled", &ls), tf(), Some(true), Some(false)));
        }
        for s in list {
            c.add(s.group(browser_group.clone()).closed(&procs));
        }

        for (pname, pdir) in chromium_profiles(&dir) {
            let prefs = pdir.join("Preferences");
            let pid = pdir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let id = |k: &str| format!("{}{tag}@{pid}.{k}", fam.id);
            let group = format!("Profile: {pname}{tag}");
            let mut list: Vec<Setting> = vec![
                setting(id("suggest"), fam.id, S::Browsers, C::Tracking, "Search suggestions",
                    format!("Sends what you type in the address bar to your search engine as you type. {note}"))
                    .boolean(js("search.suggest_enabled", &prefs), tf(), Some(true), None),
                setting(id("errorpages"), fam.id, S::Browsers, C::Tracking, "Suggestions for pages that cannot be found",
                    format!("Sends the address of pages that fail to load to a web service. {note}"))
                    .boolean(js("alternate_error_pages.enabled", &prefs), tf(), Some(true), Some(false)),
                setting(id("spelling"), fam.id, S::Browsers, C::Telemetry, "Enhanced (online) spell check",
                    format!("Sends text you type in web forms to an online spell checker. {note}"))
                    .boolean(js("spellcheck.use_spelling_service", &prefs), tf(), Some(false), Some(false)),
                setting(id("prediction"), fam.id, S::Browsers, C::Network, "Preload pages",
                    format!("Connects to, and may load, pages you have not opened yet. {note}"))
                    .choice(js("net.network_prediction_options", &prefs), &[
                        ("0", "Preload on any network", Some("0")),
                        ("1", "Browser default", Some("1")),
                        ("2", "Never preload", Some("2")),
                    ], Some("1"), Some("2")),
                setting(id("webrtc"), fam.id, S::Browsers, C::Network, "WebRTC IP address handling",
                    format!("Controls which network addresses video-call and peer-to-peer features show to websites. {note}"))
                    .choice(js("webrtc.ip_handling_policy", &prefs), &[
                        ("default", "Default (local addresses can be seen)", Some("\"default\"")),
                        ("default_public_and_private_interfaces", "Public and private address", Some("\"default_public_and_private_interfaces\"")),
                        ("default_public_interface_only", "Public address only", Some("\"default_public_interface_only\"")),
                        ("disable_non_proxied_udp", "Only through a proxy", Some("\"disable_non_proxied_udp\"")),
                    ], Some("default"), Some("default_public_interface_only")),
                setting(id("cookies"), fam.id, S::Browsers, C::Tracking, "Third-party cookies",
                    format!("Cookies from other sites embedded in a page can follow you across the web. {note}"))
                    .choice(js("profile.cookie_controls_mode", &prefs), &[
                        ("0", "Allow", Some("0")),
                        ("1", "Block in private windows", Some("1")),
                        ("2", "Block", Some("2")),
                    ], Some("1"), Some("2")),
            ];
            if fam.brave {
                list.push(setting(id("webdiscovery"), fam.id, S::Browsers, C::Tracking, "Web Discovery Project",
                    format!("Sends information about pages you visit to Brave Search to improve its index. {note}"))
                    .boolean(js("brave.web_discovery_enabled", &prefs), tf(), Some(false), Some(false)));
                list.push(setting(id("sponsored"), fam.id, S::Browsers, C::Tracking, "Sponsored images on the new tab page",
                    format!("Shows advertising images on new tabs. {note}"))
                    .boolean(js("brave.new_tab_page.show_branded_background_image", &prefs), tf(), Some(true), Some(false)));
            }
            for s in list {
                c.add(s.group(group.clone()).closed(&procs));
            }
        }
    }
    if !any {
        c.add(setting(format!("{}.noprofile", fam.id), fam.id, S::Browsers, C::Telemetry, "No profile yet",
            format!("{} has not been started yet, so there are no profile settings to show.", fam.name)).info("Start it once, then refresh"));
    }
}
