//! End-to-end test on a fake home and root under build/test-home (never /tmp):
//! discover apps, read settings, change them, undo, and restore everything.

use std::fs;
use std::path::{Path, PathBuf};

use privacy_core::admin::{handle_request, AdminRequest, AdminResponse};
use privacy_core::engine::{self, AdminExecutor};
use privacy_core::providers::collect_all;
use privacy_core::{Current, Env, Section, Value};

fn base(name: &str) -> PathBuf {
    let b = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/test-home").join(name);
    let _ = fs::remove_dir_all(&b);
    b
}

fn put(p: &Path, text: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

const PREFS: &str = "// Mozilla User Preferences\nuser_pref(\"browser.startup.page\", 3);\nuser_pref(\"datareporting.healthreport.uploadEnabled\", true);\n";
const VLCRC: &str = "[qt] # Qt interface\n#qt-recentplay=1\n\n[core] # core program\n#metadata-network-access=1\n";
const XCU: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<oor:items xmlns:oor=\"http://openoffice.org/2001/registry\">\n<item oor:path=\"/org.openoffice.Setup/Product\"><prop oor:name=\"ooSetupLastVersion\" oor:op=\"fuse\"><value>26.2</value></prop></item>\n</oor:items>\n";

fn setup(b: &Path) -> Env {
    let env = Env::for_test(b);
    let apps = env.sys("/usr/share/applications");
    let desk = |id: &str, name: &str, exec: &str| {
        put(&apps.join(format!("{id}.desktop")), &format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nIcon={id}\n"));
    };
    desk("firefox", "Firefox", "/usr/lib/firefox/firefox %u");
    desk("vlc", "VLC media player", "/usr/bin/vlc %U");
    desk("libreoffice-startcenter", "LibreOffice", "libreoffice");
    desk("org.example.Unknown", "Some Unknown App", "unknown-app");
    desk("brave-browser", "Brave", "brave %U");
    put(&env.sys("/usr/bin/firefox"), "");
    put(&env.sys("/usr/bin/brave"), "");
    put(&env.sys("/usr/bin/plasmashell"), "");
    put(&env.sys("/usr/bin/NetworkManager"), "");

    let ff = env.home.join(".mozilla/firefox");
    put(&ff.join("profiles.ini"), "[General]\nStartWithLastProfile=1\n\n[Profile0]\nName=default-release\nIsRelative=1\nPath=abc.default-release\nDefault=1\n");
    put(&ff.join("abc.default-release/prefs.js"), PREFS);

    put(&env.config.join("vlc/vlcrc"), VLCRC);
    put(&env.config.join("libreoffice/4/user/registrymodifications.xcu"), XCU);
    put(&env.config.join("baloofilerc"), "[Basic Settings]\nIndexing-Enabled=true\n");

    let brave = env.config.join("BraveSoftware/Brave-Browser");
    put(&brave.join("Local State"), r#"{"brave":{"p3a":{"enabled":true}},"profile":{"info_cache":{"Default":{"name":"Person 1"}}}}"#);
    put(&brave.join("Default/Preferences"), r#"{"search":{"suggest_enabled":true}}"#);

    let fp = env.data.join("flatpak/app/org.example.Sandboxed/current/active");
    put(&fp.join("metadata"), "[Application]\nname=org.example.Sandboxed\n\n[Context]\nshared=network;ipc;\nsockets=x11;wayland;pulseaudio;\nfilesystems=home;xdg-download:ro;\n");
    put(&env.data.join("flatpak/exports/share/applications/org.example.Sandboxed.desktop"),
        "[Desktop Entry]\nType=Application\nName=Sandboxed Thing\nExec=/usr/bin/flatpak run org.example.Sandboxed\nX-Flatpak=org.example.Sandboxed\n");
    env
}

/// Runs root operations directly against the fake root, like the helper would.
struct FakeHelper(Env);
impl AdminExecutor for FakeHelper {
    fn run(&self, req: &AdminRequest) -> Result<AdminResponse, String> {
        Ok(handle_request(&self.0, req))
    }
}

fn value(env: &Env, id: &str) -> Current {
    let m = collect_all(env);
    m.setting(id).unwrap_or_else(|| panic!("no setting {id}")).current.clone()
}

#[test]
fn discover_read_change_undo_restore() {
    let b = base("e2e");
    let env = setup(&b);
    let model = collect_all(&env);

    // Every installed app is listed, including the one nobody knows.
    for id in ["firefox", "vlc", "libreoffice", "org.example.Unknown", "brave", "org.example.Sandboxed", "kde-plasma"] {
        assert!(model.app(id).is_some(), "missing app {id}: {:?}", model.apps.iter().map(|a| &a.id).collect::<Vec<_>>());
    }
    assert_eq!(model.settings_for_app("org.example.Unknown").count(), 0);
    assert!(model.app("org.example.Sandboxed").unwrap().name == "Sandboxed Thing");

    let tele = "firefox@abc.default-release.datareporting.healthreport.uploadEnabled";
    assert_eq!(model.setting(tele).unwrap().current, Current::Value { value: Value::Bool(true), is_default: false });
    let vlc = model.setting("vlc.metadata-network-access").unwrap();
    assert_eq!(vlc.current, Current::Value { value: Value::Bool(false), is_default: true });
    assert_eq!(model.setting("brave.p3a").unwrap().current.value(), Some(&Value::Bool(true)));
    assert!(model.setting("kde.baloo").is_some());
    assert!(model.setting("flatpak.org.example.Sandboxed.shared.network").is_some());
    assert!(model.setting("flatpak.org.example.Sandboxed.filesystems.xdg-download").is_some());
    assert!(model.setting("nm.connectivity").is_some());
    assert!(model.setting("policy.brave.BraveP3AEnabled").is_some());
    assert!(model.settings.iter().any(|s| s.section == Section::Browsers));

    // Apply everything recommended in one batch (user files + fake root).
    let helper = FakeHelper(env.clone());
    let changes = engine::plan_recommended(&model, model.settings.iter());
    assert!(changes.len() > 20, "only {} changes", changes.len());
    let rep = engine::apply(&env, &changes, &helper, None);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);

    let user_js = fs::read_to_string(b.join("home/.mozilla/firefox/abc.default-release/user.js")).unwrap();
    assert!(user_js.contains("user_pref(\"datareporting.healthreport.uploadEnabled\", false);"));
    let prefs = fs::read_to_string(b.join("home/.mozilla/firefox/abc.default-release/prefs.js")).unwrap();
    assert!(prefs.contains("user_pref(\"browser.startup.page\", 3);"), "unrelated pref kept");
    assert_eq!(value(&env, tele).value(), Some(&Value::Bool(false)));
    assert_eq!(value(&env, "vlc.metadata-network-access").value(), Some(&Value::Bool(false)));
    let m = collect_all(&env);
    let ch = engine::plan(&m, m.setting("vlc.recentplay").unwrap(), &Value::Bool(false)).unwrap();
    assert!(engine::apply(&env, &[ch], &helper, None).errors.is_empty());
    assert_eq!(value(&env, "libreoffice.removepersonalinfo").value(), Some(&Value::Bool(true)));
    assert_eq!(value(&env, "brave.p3a").value(), Some(&Value::Bool(false)));
    assert_eq!(value(&env, "nm.connectivity").value(), Some(&Value::Bool(false)));
    // Policies carry no recommendation; set one explicitly through the fake helper.
    assert_eq!(value(&env, "policy.brave.BraveP3AEnabled").value(), Some(&Value::Bool(false)));
    let m = collect_all(&env);
    let pol = m.setting("policy.brave.BraveP3AEnabled").unwrap();
    let ch = engine::plan(&m, pol, &Value::Bool(true)).unwrap();
    assert!(engine::apply(&env, &[ch], &helper, None).errors.is_empty());
    assert_eq!(value(&env, "policy.brave.BraveP3AEnabled").value(), Some(&Value::Bool(true)));
    assert!(fs::read_to_string(env.sys("/etc/brave/policies/managed/privacy-panel.json")).unwrap().contains("\"BraveP3AEnabled\": false"));

    // A flatpak permission off and on.
    let m = collect_all(&env);
    let net = m.setting("flatpak.org.example.Sandboxed.shared.network").unwrap();
    let ch = engine::plan(&m, net, &Value::Bool(false)).unwrap();
    assert!(engine::apply(&env, &[ch], &helper, None).errors.is_empty());
    let ov = fs::read_to_string(env.data.join("flatpak/overrides/org.example.Sandboxed")).unwrap();
    assert!(ov.contains("shared=!network;"), "{ov}");
    assert_eq!(value(&env, "flatpak.org.example.Sandboxed.shared.network").value(), Some(&Value::Bool(false)));

    // Undo the last batch (the flatpak change).
    let batches = privacy_core::history::batches(&env);
    let rep = engine::undo_batch(&env, batches[0].id, &helper);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(value(&env, "flatpak.org.example.Sandboxed.shared.network").value(), Some(&Value::Bool(true)));

    // A running app blocks its settings.
    let mut env_run = env.clone();
    env_run.processes = Some(["vlc".to_string()].into_iter().collect());
    let m = collect_all(&env_run);
    let v = m.setting("vlc.metadata-network-access").unwrap();
    assert_eq!(v.blocked_by.as_deref(), Some("VLC media player"));
    assert!(engine::plan(&m, v, &Value::Bool(true)).is_err());

    // Restore everything: files are back to how they were.
    let rep = engine::restore_everything(&env, &helper);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(fs::read_to_string(env.config.join("vlc/vlcrc")).unwrap(), VLCRC);
    assert_eq!(fs::read_to_string(env.config.join("libreoffice/4/user/registrymodifications.xcu")).unwrap(), XCU);
    assert_eq!(fs::read_to_string(b.join("home/.mozilla/firefox/abc.default-release/prefs.js")).unwrap(), PREFS);
    assert_eq!(value(&env, tele).value(), Some(&Value::Bool(true)));
    assert_eq!(value(&env, "nm.connectivity").value(), Some(&Value::Bool(true)));
    let ff_user = fs::read_to_string(b.join("home/.mozilla/firefox/abc.default-release/user.js")).unwrap_or_default();
    assert!(!ff_user.contains("privacy-panel"), "managed block removed: {ff_user}");
    assert!(env.app_state().join("backups").exists());
}

/// A helper that answers with too few results must not crash the program.
struct ShortHelper;
impl AdminExecutor for ShortHelper {
    fn run(&self, _req: &AdminRequest) -> Result<AdminResponse, String> {
        Ok(AdminResponse { results: vec![] })
    }
}

#[test]
fn short_helper_answer_is_an_error_not_a_crash() {
    let b = base("short");
    let env = setup(&b);
    let m = collect_all(&env);
    let s = m.setting("nm.connectivity").unwrap();
    let ch = engine::plan(&m, s, &Value::Bool(false)).unwrap();
    let rep = engine::apply(&env, &[ch], &ShortHelper, None);
    assert_eq!(rep.errors.len(), 1);
    assert!(rep.applied.is_empty());
}

#[test]
fn symlinked_config_stays_a_link() {
    let b = base("symlink");
    let env = setup(&b);
    let real = b.join("dotfiles/vlcrc");
    put(&real, VLCRC);
    let link = env.config.join("vlc/vlcrc");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let m = collect_all(&env);
    let s = m.setting("vlc.metadata-network-access").unwrap();
    let ch = engine::plan(&m, s, &Value::Bool(true)).unwrap();
    let helper = FakeHelper(env.clone());
    assert!(engine::apply(&env, &[ch], &helper, None).errors.is_empty());
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert!(fs::read_to_string(&real).unwrap().contains("metadata-network-access=1"));
}

#[test]
fn every_setting_explains_itself() {
    let b = base("about");
    let env = setup(&b);
    let m = collect_all(&env);
    for s in &m.settings {
        assert!(s.about.is_some(), "no \"What is this?\" text for {}", s.id);
        if matches!(s.kind, privacy_core::Kind::Bool | privacy_core::Kind::Choice(_)) {
            let c = privacy_core::describe::change(&env, s).unwrap();
            assert!(!c.is_empty(), "no change description for {}", s.id);
        }
    }
    let s = m.setting("vlc.metadata-network-access").unwrap();
    let c = privacy_core::describe::change(&env, s).unwrap();
    assert!(c.contains("~/.config/vlc/vlcrc") && c.contains("Off: writes metadata-network-access=0"), "{c}");
    let f = m.setting("flatpak.org.example.Sandboxed.filesystems.home").unwrap();
    assert!(privacy_core::describe::change(&env, f).unwrap().contains("--nofilesystem=home org.example.Sandboxed"));
}
