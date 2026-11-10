//! Where things live on this machine. Every path the providers use goes through `Env`,
//! so tests can point the whole program at a fake home and a fake root.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Env {
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    /// Prefix for system paths such as /etc and /usr. "/" on a real system.
    pub root: PathBuf,
    /// When false, no external commands are run and no D-Bus calls are made (tests).
    pub live: bool,
    /// Test override for the list of running process names.
    pub processes: Option<HashSet<String>>,
    /// Lower-case XDG_CURRENT_DESKTOP, e.g. "kde" or "gnome".
    pub desktop: String,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

impl Env {
    pub fn from_system() -> Env {
        let home = env_path("HOME").unwrap_or_else(|| PathBuf::from("/root"));
        Env {
            config: env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            data: env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
            cache: env_path("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache")),
            state: env_path("XDG_STATE_HOME").unwrap_or_else(|| home.join(".local/state")),
            home,
            root: PathBuf::from("/"),
            live: true,
            processes: None,
            desktop: std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .to_lowercase(),
        }
    }

    /// An environment rooted in a test folder: `<base>/home` and `<base>/root`.
    pub fn for_test(base: &Path) -> Env {
        let home = base.join("home");
        Env {
            config: home.join(".config"),
            data: home.join(".local/share"),
            cache: home.join(".cache"),
            state: home.join(".local/state"),
            home,
            root: base.join("root"),
            live: false,
            processes: Some(HashSet::new()),
            desktop: "kde".into(),
        }
    }

    /// A system path such as "/etc/foo" under the configured root.
    pub fn sys(&self, path: &str) -> PathBuf {
        self.root.join(path.trim_start_matches('/'))
    }

    /// Expand `~`, `$HOME`, `$XDG_CONFIG_HOME`, `$XDG_DATA_HOME`, `$XDG_CACHE_HOME`,
    /// `$XDG_STATE_HOME`. Absolute paths are taken as system paths under `root`.
    pub fn expand(&self, s: &str) -> PathBuf {
        let pairs: [(&str, &Path); 6] = [
            ("$XDG_CONFIG_HOME", &self.config),
            ("$XDG_DATA_HOME", &self.data),
            ("$XDG_CACHE_HOME", &self.cache),
            ("$XDG_STATE_HOME", &self.state),
            ("$HOME", &self.home),
            ("~", &self.home),
        ];
        for (var, base) in pairs {
            if let Some(rest) = s.strip_prefix(var) {
                return base.join(rest.trim_start_matches('/'));
            }
        }
        if s.starts_with('/') {
            self.sys(s)
        } else {
            self.home.join(s)
        }
    }

    /// The app's own state folder (history, backups).
    pub fn app_state(&self) -> PathBuf {
        self.state.join("privacy-panel")
    }

    pub fn is_gnome(&self) -> bool {
        self.desktop.contains("gnome")
            || self.desktop.contains("unity")
            || self.desktop.contains("budgie")
            || self.desktop.contains("cinnamon")
            || self.sys("/usr/bin/gnome-shell").exists()
    }

    pub fn is_kde(&self) -> bool {
        self.desktop.contains("kde") || self.sys("/usr/bin/plasmashell").exists()
    }

    /// Find an executable in the usual system bin folders (under root).
    pub fn has_binary(&self, name: &str) -> bool {
        if name.starts_with('/') {
            return self.sys(name).exists();
        }
        ["/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin", "/opt/bin"]
            .iter()
            .any(|d| self.sys(d).join(name).exists())
    }
}
