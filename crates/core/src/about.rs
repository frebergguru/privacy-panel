//! Plain-language descriptions: what each thing is, for settings, apps, pages and
//! categories. Shown behind the "What is this?" button in the GUI.

use crate::model::{Category, Section};

/// (pattern, text). A pattern starting with '*' matches the end of the setting id, one
/// ending with '*' matches the start, anything else matches anywhere. First match wins,
/// so specific patterns come before general ones.
const SETTINGS: &[(&str, &str)] = &[
    // ---- KDE
    ("kde.baloo-basic", "Baloo can index only file names, or also what is inside your files (the text of documents, tags of music and so on). Indexing contents makes search find more, but it also means a searchable copy of your documents' text is kept in ~/.local/share/baloo."),
    ("kde.baloo", "Baloo is KDE's file search engine. It runs in the background and builds an index (a kind of catalogue) of the files in your home folder, so that search in the app launcher and in Dolphin is fast. The index is stored in ~/.local/share/baloo and is never sent anywhere."),
    ("kde.userfeedback", "KUserFeedback is the system KDE apps use to collect usage statistics, such as which version you run and which features you use, and to show the occasional survey. It is off unless you allow it. This switch is the master switch for all KDE apps at once."),
    ("kde.feedback-level", "The feedback level decides how much Plasma tells KDE when user feedback is allowed: from nothing, to basic facts about your system (versions, screen size, graphics card), to details about which features you use. The data helps KDE's developers and is sent to KDE's own server."),
    ("kde.activity-remember", "Plasma's activity manager keeps a private log of which files you opened with which apps and when. It powers \"Recent files\" in the launcher, the task manager's jump lists and search ranking. The log stays on this computer, but anyone using your account can see it."),
    ("kde.activity-keep", "How long Plasma's activity manager keeps its log of which files and apps you used."),
    ("kde.recent-docs", "KDE apps such as Kate, Okular and Gwenview remember the documents you opened, for their \"Open Recent\" menus."),
    ("kde.clipboard-keep", "Klipper is Plasma's clipboard manager. It keeps a history of what you copied, so you can paste older items. With this on, the history is saved to disk when you log out and comes back next time, including any passwords you copied."),
    ("kde.autolock", "The screen locker covers your screen and asks for your password after you have been away for a while."),
    ("kde.lock-resume", "When the computer wakes from sleep, the screen locker asks for your password before showing your desktop."),
    ("kde.wallet-screensaver", "KWallet is KDE's password safe. Apps such as Wi-Fi settings and browsers store passwords in it. While it is open, any app running as you can ask it for those passwords."),
    // ---- KDE Connect
    ("kdeconnect.devices", "KDE Connect links your phone or another computer with this one over your local network, to share files, the clipboard, notifications and more. A device must be paired (accepted on both sides) before it can connect."),
    ("*kdeconnect_clipboard", "KDE Connect's clipboard sharing copies everything you copy on one device to the other one automatically."),
    ("*kdeconnect_sendnotifications", "KDE Connect can show this computer's notifications on the paired phone."),
    ("*kdeconnect_mousepad", "\"Remote input\" lets the paired device act as a touchpad and keyboard for this computer."),
    ("*kdeconnect_runcommand", "\"Run command\" lets the paired device start commands that you have set up on this computer, for example to shut it down."),
    ("*kdeconnect_sftp", "This lets this computer browse the files on the paired phone, through a secure file transfer connection."),
    // ---- GNOME and GTK
    ("gnome.org.gnome.system.location.enabled", "Location services let apps ask where you are. GNOME uses GeoClue for this, which usually works out your position by sending the Wi-Fi networks around you to an online location service."),
    ("gnome.org.gnome.desktop.screensaver.lock-enabled", "The screen lock covers your screen and asks for your password when the screen saver starts."),
    ("report-technical-problems", "Problem reporting sends information about crashes and errors to your distribution's developers, so they can fix them. Reports can contain details about what you were doing."),
    ("send-software-usage-stats", "Usage statistics are anonymous counts of which software you use, sent to help developers decide what to work on."),
    ("remember-recent-files", "GNOME apps keep a list of files you opened recently, shown in file dialogs and in Files."),
    ("remember-app-usage", "GNOME keeps track of how often you use each app, to show your favourites first."),
    ("remove-old-trash-files", "Files you delete go to the trash first. With this on, GNOME empties old files from the trash automatically."),
    ("remove-old-temp-files", "Apps leave temporary files behind. With this on, GNOME cleans old ones up automatically."),
    ("disable-camera", "A switch that stops every app from using the camera, without unplugging it."),
    ("disable-microphone", "A switch that stops every app from using the microphone."),
    ("gnome.notif-lock", "Notifications are the messages that pop up from apps, such as new email or chat messages."),
    ("gnome.recent-age", "How long files stay in GNOME's list of recently used files."),
    ("gnome.search-external", "The Activities overview search can ask installed apps for results, so typing a word can show matching files, contacts, software and web results."),
    ("gtk.recent", "Apps built with GTK (such as GIMP, Inkscape and many GNOME apps) share one list of recently used files, stored in ~/.local/share/recently-used.xbel."),
    ("cleanup.recent", "The recent files list is shared by GTK and KDE apps. It lists the names and locations of files you opened, so others using your account could see what you worked on."),
    ("cleanup.thumbs", "Thumbnails are the small preview pictures file managers show. They are saved in ~/.cache/thumbnails and stay there even after the original pictures and videos are deleted."),
    // ---- Firefox family
    ("datareporting.healthreport.uploadEnabled", "Telemetry is data a program sends about itself: how fast it is, what hardware it runs on, which features are used, and errors. Mozilla uses it to improve Firefox. It does not include the pages you visit, but it is still information about you and your computer."),
    ("app.shield.optoutstudies.enabled", "Studies are experiments Mozilla runs on a share of Firefox users: a feature is turned on, changed or tested on your copy, and the results are reported back."),
    ("browser.discovery.enabled", "Firefox can suggest extensions in the Add-ons Manager based on how you use the browser."),
    ("browser.crashReports.unsubmittedCheck.autoSubmit2", "When Firefox crashes, it saves a crash report. With this on, saved reports are sent to Mozilla without asking you."),
    ("browser.newtabpage.activity-stream.feeds.telemetry", "The new tab page (\"Firefox Home\") can report which parts of it you click and use."),
    ("browser.newtabpage.activity-stream.showSponsoredTopSites", "Sponsored shortcuts are paid tiles that appear among your top sites on the new tab page."),
    ("browser.newtabpage.activity-stream.showSponsored", "Sponsored stories are paid articles shown among recommended stories on the new tab page."),
    ("browser.urlbar.suggest.quicksuggest.sponsored", "Firefox Suggest shows extra results in the address bar as you type. Sponsored ones are paid advertising."),
    ("browser.urlbar.suggest.quicksuggest.nonsponsored", "Firefox Suggest shows results from Wikipedia and other partners in the address bar as you type."),
    ("extensions.pocket.enabled", "Pocket is a Mozilla service for saving articles to read later, built into Firefox with a button and stories on the new tab page."),
    ("dom.private-attribution.submission.enabled", "Privacy-preserving attribution is a Mozilla experiment that lets advertisers learn whether ads worked, using combined, anonymous reports instead of tracking cookies."),
    ("privacy.trackingprotection.enabled", "Trackers are scripts, often from advertising companies, that are built into many websites and follow you from site to site. Tracking protection blocks the known ones."),
    ("privacy.globalprivacycontrol.enabled", "Global Privacy Control (GPC) is a signal your browser sends with every page request saying \"do not sell or share my data\". In some places, such as California, sites are required by law to respect it."),
    ("dom.security.https_only_mode", "HTTPS is the encrypted version of the web protocol (the padlock in the address bar). Plain HTTP can be read and changed by anyone on the network between you and the site."),
    ("media.peerconnection.ice.default_address_only", "WebRTC is the technology behind video calls in the browser. To connect directly, it can tell websites your local network addresses (such as 192.168.1.20), which can be used to identify you even behind a VPN."),
    ("network.prefetch-next", "Link prefetching means a web page can tell the browser to download pages it thinks you will open next, before you click."),
    ("network.dns.disablePrefetch", "DNS is the internet's phone book: it turns names like example.com into addresses. DNS prefetching looks up every link on a page in advance, which tells your DNS provider about links you never clicked."),
    ("browser.safebrowsing.downloads.remote.enabled", "Safe Browsing checks sites and downloads against Google's lists of dangerous ones. This part sends details of some downloaded programs to Google to check them online."),
    ("network.captive-portal-service.enabled", "A captive portal is the login page some hotel, airport and café Wi-Fi networks show before letting you online. Firefox detects them by contacting Mozilla's server now and then."),
    ("network.connectivity-service.enabled", "Firefox checks now and then, by contacting Mozilla, whether your connection supports IPv4 and IPv6, to pick the best way to connect."),
    ("browser.search.suggest.enabled", "Search suggestions are the guesses shown under the address bar as you type, supplied by your search engine. To make them, everything you type there is sent to the search engine as you type it."),
    ("geo.enabled", "The geolocation feature lets websites ask for your position, for example to show maps or local weather. Firefox works it out from nearby Wi-Fi networks using an online service."),
    ("browser.ml.chat.enabled", "Firefox can show an AI chatbot (such as ChatGPT or Claude) in the sidebar, to summarise or explain pages you send to it."),
    ("*.trr", "DNS is the internet's phone book: it turns names like example.com into addresses. Normally these look-ups are unencrypted, so your network and internet provider can see every site name you visit. DNS over HTTPS encrypts them and sends them to a DNS provider you choose instead."),
    ("mailnews.message_display.disable_remote_image", "Emails can contain pictures that are loaded from the internet when you open the message. Senders use invisible ones (tracking pixels) to see when, where and how often you read their email."),
    ("mailnews.start_page.enabled", "Thunderbird can show a web page from Mozilla when it starts."),
    ("mailnews.headers.sendUserAgent", "Every email carries hidden headers. The User-Agent header names the program and system that sent it."),
    ("privacy.donottrackheader.enabled", "Do Not Track is an old signal asking sites not to track you. Most sites ignore it."),
    // ---- Chromium family (ids end with these)
    ("*.metrics", "Usage statistics and crash reports tell the browser's maker how the browser is used and when it crashes. They can include which features you use and details about your system."),
    ("*.background", "Some extensions and web apps can keep running after you close the browser. The browser then keeps running too, with only an icon in the system tray."),
    ("*.p3a", "P3A (Privacy-Preserving Product Analytics) is Brave's way of collecting usage statistics: answers to fixed questions, such as how many tabs you have open, sent without identifiers."),
    ("*.stats", "Brave's usage ping is a small daily message that lets Brave count how many people use the browser, and which promotion they came from."),
    ("*.suggest", "Search suggestions are the guesses shown under the address bar as you type, supplied by your search engine. To make them, everything you type there is sent to the search engine."),
    ("*.errorpages", "When a page cannot be found, the browser can ask a web service for similar addresses that might be what you meant."),
    ("*.spelling", "The enhanced spell checker sends the text you type into web forms to an online service, which gives better suggestions than the built-in local one."),
    ("*.prediction", "Preloading means the browser connects to sites, and sometimes loads whole pages, it thinks you will open next. Sites you never visit can then see your IP address."),
    ("*.webrtc", "WebRTC is the technology behind video calls in the browser. To connect directly, it can tell websites your local network addresses (such as 192.168.1.20), which can be used to identify you even behind a VPN."),
    ("*.cookies", "Cookies are small pieces of data sites store in your browser. Third-party cookies come from other companies built into the page, such as advertisers, and let them follow you from site to site."),
    ("*.webdiscovery", "The Web Discovery Project sends Brave information about pages you visit, with personal details removed, to help Brave Search build its own index of the web."),
    ("*.sponsored", "Brave's new tab page can show sponsored background images, which are advertising."),
    // ---- Browser policies (by policy name)
    ("MetricsReportingEnabled", "Usage statistics and crash reports tell the browser's maker how the browser is used and when it crashes."),
    ("UrlKeyedAnonymizedDataCollectionEnabled", "\"Make searches and browsing better\" sends the address of each page you visit to the browser maker, to show you suggestions."),
    ("SafeBrowsingExtendedReportingEnabled", "Safe Browsing checks sites against lists of dangerous ones. Extended reporting also uploads samples of pages and downloads you come across."),
    ("AlternateErrorPagesEnabled", "When a page cannot be found, the browser can ask a web service for similar addresses."),
    ("SpellCheckServiceEnabled", "The enhanced spell checker sends what you type in web forms to an online service."),
    ("SearchSuggestEnabled", "Search suggestions send what you type in the address bar to your search engine as you type."),
    ("NetworkPredictionOptions", "Preloading lets the browser connect to and load pages before you click them."),
    ("WebRtcIPHandling", "WebRTC (browser video calls) can reveal your local network addresses to websites."),
    ("BlockThirdPartyCookies", "Third-party cookies let companies built into many sites, such as advertisers, follow you from site to site."),
    ("BackgroundModeEnabled", "Background mode keeps the browser running after its last window is closed."),
    ("PrivacySandbox", "The Privacy Sandbox is Google's replacement for tracking cookies: the browser itself works out your interests and lets advertisers use them, without the advertiser seeing your history."),
    ("BraveP3AEnabled", "P3A is Brave's anonymous usage statistics."),
    ("BraveStatsPingEnabled", "The usage ping is a small daily message Brave uses to count active users."),
    ("BraveWebDiscoveryEnabled", "Web Discovery sends information about the pages you visit to Brave Search."),
    ("BraveRewardsDisabled", "Brave Rewards pays you in cryptocurrency for viewing Brave's own private ads."),
    ("BraveWalletDisabled", "Brave Wallet is a cryptocurrency wallet built into the browser."),
    ("BraveVPNDisabled", "Brave VPN is a paid VPN service offered inside the browser."),
    ("BraveAIChatEnabled", "Leo is Brave's AI assistant, which can answer questions and summarise pages."),
    ("BraveNewsDisabled", "Brave News is a news feed shown on the new tab page."),
    ("DisableTelemetry", "Telemetry is data Firefox sends to Mozilla about its performance, hardware and how features are used."),
    ("DisableFirefoxStudies", "Studies are experiments Mozilla runs on a share of Firefox users."),
    ("DisablePocket", "Pocket is a save-for-later service built into Firefox."),
    ("SponsoredSuggestions", "Firefox Suggest can show paid results in the address bar."),
    ("ImproveSuggest", "Firefox can send what you type in the address bar to Mozilla to improve its suggestions."),
    ("policies.NetworkPrediction", "DNS prefetching and preconnecting look up and connect to links before you click them."),
    ("CaptivePortal", "Captive portal detection finds hotel and airport Wi-Fi login pages by contacting Mozilla now and then."),
    ("EnableTrackingProtection", "Tracking protection blocks known trackers, the scripts that follow you from site to site."),
    // ---- Apps
    ("vlc.metadata-network-access", "When you play a song or film, VLC can search online databases using its title and artist, to show cover art and details. The services then learn what you are playing."),
    ("vlc.recentplay", "VLC remembers the files and streams you played, in the Media > Open Recent Media menu."),
    ("libreoffice.updatecheck", "LibreOffice can check its website for new versions now and then."),
    ("libreoffice.removepersonalinfo", "Office documents carry hidden information (metadata): who wrote and edited them, when, for how long, and who made each comment and change. Anyone you send the file to can read it in File > Properties."),
    ("libreoffice.crashreport", "When LibreOffice crashes, it can upload a crash report with technical details to its developers."),
    ("libreoffice.usage", "LibreOffice can collect anonymous statistics about which features you use."),
    ("libreoffice.macros", "Macros are small programs stored inside documents that automate tasks. Harmful documents use them to run malware when opened."),
    ("libreoffice.picklist", "LibreOffice remembers recently opened documents, shown in the Start Center and in File > Recent Documents."),
    ("gimp.check-updates", "GIMP can check gimp.org for a new version each time it starts."),
    ("audacity.updates", "Audacity can check its website for a new version each time it starts."),
    ("transmission.dht", "BitTorrent is a way of sharing files directly between many computers. DHT is a worldwide shared list of who is sharing what, used to find others without a central server."),
    ("transmission.pex", "Peer exchange (PEX) lets BitTorrent clients tell each other about other people sharing the same files."),
    ("transmission.lpd", "Local Peer Discovery (LPD) finds others sharing the same torrent on your own network by announcing it to everyone on that network."),
    ("transmission.encryption", "BitTorrent traffic can be encrypted so that your internet provider cannot easily see what you transfer. Others in the swarm still see your IP address."),
    ("transmission.blocklist", "A blocklist is a downloaded list of IP addresses that Transmission refuses to connect to."),
    ("heroic.analytics", "Heroic is a launcher for games from Epic, GOG and Amazon. Its analytics are anonymous usage statistics sent to the Heroic developers."),
    ("filezilla.kiosk", "FileZilla transfers files to and from servers (FTP and SFTP). It can save each server's password so you do not have to type it again."),
    ("vscode.telemetry", "Telemetry is data VS Code sends to Microsoft about how it is used, errors and crashes."),
    ("vscodium.telemetry", "VSCodium is VS Code built without Microsoft's telemetry, but extensions can still send their own data."),
    (".experiments", "Experiments let Microsoft turn features on or off remotely for some users, to test them."),
    (".nlsearch", "Natural language search sends what you type in the Settings search box to an online service."),
    (".npm", "npm is the package registry for JavaScript. When you open a package.json file, the editor can look up details of the packages it names online."),
    // ---- Flatpak sandbox permissions (ids are flatpak.<app>.<key>.<token>)
    ("*.shared.network", "Network access lets the app use the internet and your local network. Without it the app works offline only."),
    ("*.sockets.x11", "X11 is the older system for showing windows on Linux. Apps with X11 access can see other X11 windows, take screenshots of them and read keys typed into them. Wayland, the newer system, does not allow that."),
    ("*.sockets.fallback-x11", "X11 is the older system for showing windows on Linux. Apps with X11 access can watch other X11 windows and keystrokes. \"Fallback\" means it is only used when Wayland is not available."),
    ("*.sockets.pulseaudio", "PulseAudio (today provided by PipeWire) is the sound system. Access to it lets the app play sound, but also record from any microphone."),
    ("*.sockets.session-bus", "D-Bus is the message system programs in your session use to talk to each other. Full access lets the app talk to any other program as you, which mostly undoes the sandbox."),
    ("*.sockets.system-bus", "The system bus is the D-Bus message system for system services such as NetworkManager and the login manager."),
    ("*.sockets.ssh-auth", "The SSH agent holds your unlocked SSH keys, used to log in to servers and Git hosts."),
    ("*.sockets.gpg-agent", "The GPG agent holds your unlocked GPG keys, used for encrypting and signing."),
    ("*.sockets.cups", "CUPS is the printing system."),
    ("*.sockets.pcsc", "PC/SC is the system for smart card readers, used for ID cards and security keys."),
    ("*.devices.all", "\"All devices\" gives the app access to everything in /dev: webcams, USB devices, game controllers and more."),
    ("*.devices.kvm", "KVM is the Linux virtual machine system."),
    ("*.devices.shm", "Shared memory lets programs exchange data quickly through /dev/shm."),
    ("*.features.devel", "Development features let the app trace and debug other programs, which normal apps never need."),
    ("*.features.bluetooth", "Direct Bluetooth access, without going through the system's Bluetooth service."),
    ("*.filesystems.host", "\"host\" means almost the entire file system of this computer, including your home folder and other users' readable files."),
    ("*.filesystems.host-os", "System files such as programs and libraries in /usr."),
    ("*.filesystems.host-etc", "/etc holds the system's configuration files."),
    ("*.filesystems.home", "Your home folder holds all your personal files, and the settings and data of all your other apps."),
    ("flatpak.*", "Flatpak apps run in a sandbox: a fence around the app that only lets it reach the files, devices and services it has been given permission to. These are the permissions this app asked for when it was installed. Turning one off adds a local override; the app itself is not changed."),
    // ---- Portals and snaps
    ("portal.devices.camera", "The camera permission was granted when the app asked through the desktop's camera prompt."),
    ("portal.devices.microphone", "The microphone permission was granted when the app asked through the desktop's prompt."),
    ("portal.location", "The location permission was granted when the app asked where you are."),
    ("portal.background", "Apps can ask to keep running when their windows are closed, and to start automatically when you log in."),
    ("portal.screenshot", "Apps can ask to take a screenshot of the whole screen."),
    ("portal.*", "The desktop portal is the part of your desktop that asks \"Allow this app to use...?\" when an app wants something sensitive. Your answers are remembered in a permission store, so the app does not ask again. Turning one off removes that remembered yes."),
    ("snap.*", "Snap apps run in a sandbox. Interfaces are the doors in that sandbox: each one gives access to one thing, such as the camera or your home folder. Connected means the door is open."),
    // ---- System services
    ("unit.avahi", "Avahi (mDNS/DNS-SD, also called Zeroconf or Bonjour) lets computers on the same network find each other without setup. It announces this computer's name and services, such as shared folders, to every device on the network, and finds printers and media servers."),
    ("unit.cups-browsed", "cups-browsed watches the network for printers shared by other computers and adds them to your printer list automatically."),
    ("unit.bluetooth", "The Bluetooth service lets the computer use Bluetooth devices such as headphones, mice and phones. While it is on, the computer may be visible to nearby devices when you pair."),
    ("unit.sshd", "SSH (Secure Shell) is a way to log in to a computer over the network and control it from another one. The SSH server is the part that accepts those logins."),
    ("unit.ssh", "SSH (Secure Shell) is a way to log in to a computer over the network and control it from another one. The SSH server is the part that accepts those logins."),
    ("unit.modemmanager", "ModemManager controls mobile broadband modems (3G, 4G, 5G) and some USB phones."),
    ("unit.pkgstats", "pkgstats is an optional Arch Linux tool. Once a week it sends the list of installed packages, your CPU architecture and country (from your IP address) to Arch's statistics server."),
    ("unit.apport", "Apport is Ubuntu's crash reporter. When a program crashes it collects a report, which can include memory from the program."),
    ("unit.whoopsie", "Whoopsie uploads crash reports collected by Apport to Canonical, the company behind Ubuntu."),
    ("unit.abrtd", "ABRT (Automatic Bug Reporting Tool) collects crash reports on Fedora and Red Hat systems."),
    ("unit.geoclue", "GeoClue is the shared location service of the Linux desktop. Apps ask it where you are, and it works it out from GPS, nearby Wi-Fi networks (sent to an online service) or your IP address."),
    // ---- NetworkManager
    ("nm.wifi-mac", "A MAC address is the hardware address of a network card, a unique number built in at the factory. Every network you join sees it, so it can be used to recognise your computer every time you come back, and to link visits to different places."),
    ("nm.eth-mac", "A MAC address is the hardware address of a network card. Every network you join sees it, so it can be used to recognise your computer."),
    ("nm.scan-rand", "While looking for Wi-Fi networks, the computer sends out probe messages that carry its MAC address. Shops and public places can log those to track people walking past."),
    ("nm.connectivity", "NetworkManager is the program that connects your computer to Wi-Fi and wired networks. Its connectivity check regularly downloads a small file from a server run by your distribution to find out whether the internet really works."),
    ("nm.dhcp4-hostname", "DHCP is how your computer gets its address when it joins a network: it asks the router, and normally also tells the router its own name (the host name you gave the computer, for example \"annes-laptop\"). Other devices on the network may see that name."),
    ("nm.dhcp6-hostname", "The same as the IPv4 setting, for the newer IPv6 kind of network address."),
    ("nm.ip6-privacy", "IPv6 is the newer kind of internet address. Every site you visit sees it. Privacy addresses (temporary addresses) are extra random addresses that change regularly, so sites cannot follow the same address across days and networks."),
    // ---- Logs and telemetry
    ("journald.storage", "The system journal (journald) is the log where Linux and its services write down what happens: logins, connected devices, networks joined, errors and much more."),
    ("journald.retention", "The system journal (journald) keeps a log of what the system and its services do. Old entries are normally removed only when the log gets too big."),
    ("coredump.storage", "A core dump is a copy of a program's memory at the moment it crashed, saved to help developers find the bug. It contains whatever the program had in memory, such as open documents, messages or passwords."),
    ("popcon", "The popularity contest (popcon) is an optional Debian and Ubuntu package. It sends a weekly list of installed packages and how often they are used, to decide which packages matter most."),
    ("motd-news", "The message of the day is the text shown when you log in on a terminal. On Ubuntu it can include news downloaded from Canonical."),
    ("apport.enabled", "Apport is Ubuntu's crash reporter, which collects reports when programs crash."),
    ("whoopsie.report", "Whoopsie uploads crash reports to Canonical's error tracker."),
];

/// What a thing is, for one setting (by its id).
pub fn setting(id: &str) -> Option<&'static str> {
    SETTINGS.iter().find(|(pat, _)| matches(pat, id)).map(|(_, t)| *t)
}

fn matches(pat: &str, id: &str) -> bool {
    if let Some(end) = pat.strip_prefix('*') {
        id.ends_with(end)
    } else if let Some(start) = pat.strip_suffix('*') {
        id.starts_with(start)
    } else {
        id.contains(pat)
    }
}

/// Extra note shown for every browser policy.
pub const POLICY_NOTE: &str = "A browser policy is a rule set by the computer's administrator, normally used by companies and schools. The browser obeys it for every user, and the setting appears locked inside the browser with \"Managed by your organisation\". Turning the switch off removes the rule, and the browser's own setting applies again.";

/// What an app or system component is.
pub fn app(id: &str) -> Option<&'static str> {
    Some(match id {
        "kde-plasma" => "Plasma is the KDE desktop: the panel, app launcher, file search, notifications and screen locker, plus settings shared by all KDE apps.",
        "kdeconnect" => "KDE Connect links your phone or another computer with this one over your local network, to share files, the clipboard and notifications, and to use the phone as a remote control.",
        "gnome" => "GNOME is a desktop environment: the top bar, Activities overview, file search, notifications and lock screen.",
        "gtk" => "GTK is the toolkit many Linux apps are built with, such as GIMP, Inkscape, Firefox's file dialogs and most GNOME apps. Some settings are shared by all of them.",
        "cleanup" => "One-off clean-up of traces of what you have done on this computer. These are not settings: each button deletes something once.",
        "firefox" => "Firefox is a web browser made by Mozilla, a non-profit. Each profile has its own settings, history and extensions.",
        "librewolf" => "LibreWolf is a version of Firefox with telemetry removed and stricter privacy settings by default.",
        "floorp" | "waterfox" | "zen" => "A web browser based on Firefox. It uses the same settings as Firefox.",
        "thunderbird" => "Thunderbird is an email program made by the Mozilla project.",
        "brave" => "Brave is a web browser based on Chromium (the open-source part of Google Chrome), with built-in ad and tracker blocking. Brave the company also offers its own search engine, ads, VPN and AI assistant inside the browser.",
        "google-chrome" => "Google Chrome is Google's web browser. Many of its features work through Google's online services.",
        "chromium" => "Chromium is the open-source browser Google Chrome is built from, without some of Google's services.",
        "vivaldi" => "Vivaldi is a web browser based on Chromium, made by a Norwegian company.",
        "microsoft-edge" => "Microsoft Edge is Microsoft's web browser, based on Chromium.",
        "opera" => "Opera is a web browser based on Chromium.",
        "vlc" => "VLC is a media player for music, films and streams.",
        "libreoffice" => "LibreOffice is an office suite: Writer (documents), Calc (spreadsheets), Impress (presentations) and more.",
        "gimp" => "GIMP is an image editor, similar to Photoshop.",
        "audacity" => "Audacity is a sound recorder and editor.",
        "transmission" => "Transmission is a BitTorrent program: it downloads and shares files directly with other people's computers.",
        "heroic" => "Heroic is a launcher for games from Epic Games, GOG and Amazon Prime Gaming.",
        "filezilla" => "FileZilla transfers files between this computer and servers, using FTP or SFTP.",
        "vscode" => "Visual Studio Code is a code editor made by Microsoft.",
        "vscodium" => "VSCodium is Visual Studio Code built from the open-source code without Microsoft's branding and telemetry.",
        "system-services" => "Services are programs that run in the background for the whole computer, started by systemd when the computer boots. Turning one off stops it now and keeps it from starting again.",
        "networkmanager" => "NetworkManager is the program that connects this computer to Wi-Fi, wired and VPN networks. These settings decide what it reveals about the computer to the networks it joins. They are written to a file of their own, /etc/NetworkManager/conf.d/90-privacy-panel.conf; a setting made for one particular connection still wins.",
        "system-logs" => "Linux keeps logs of what happens on the computer, and copies of the memory of programs that crash. They help with fixing problems, but they also record what you did.",
        "distro-telemetry" => "Some Linux distributions include tools that send statistics or crash reports to the distribution's makers.",
        _ => return None,
    })
}

/// Fallback for apps without their own text.
pub fn app_by_source(source: crate::model::AppSource) -> Option<&'static str> {
    match source {
        crate::model::AppSource::Flatpak => Some("A Flatpak app: it runs in a sandbox and can only reach what it has been given permission for. Its permissions are listed below."),
        crate::model::AppSource::Snap => Some("A Snap app: it runs in a sandbox, and its interfaces (permissions) are listed below."),
        _ => None,
    }
}

/// Introduction at the top of each page.
pub fn section(s: Section) -> &'static str {
    match s {
        Section::System => "Settings of the desktop and of the operating system itself: file search, history, screen locking, background services, logs and data sent to your distribution.",
        Section::Network => "What this computer tells the networks it joins (Wi-Fi, home network, public hotspots), and which services it offers to other devices on them.",
        Section::Browsers => "Each browser profile has its own settings. \"Policies for all users\" are optional rules that lock a setting for everyone on this computer.",
        Section::Applications => "Privacy settings of the other apps installed on this computer.",
        Section::Permissions => "Permissions you have given apps through the desktop's \"Allow this app to...?\" prompts, such as camera, microphone and location. The desktop remembers your answer; here you can take it back.",
        Section::Sandbox => "Flatpak and Snap apps run in a sandbox, a fence that only lets them reach what they were given permission to. These are the permissions each one has. Turning one off only changes it on this computer.",
    }
}

/// What a category covers (overview page).
pub fn category(c: Category) -> &'static str {
    match c {
        Category::Telemetry => "Data apps send to their makers about how they are used, crashes and your system.",
        Category::Tracking => "Things that let companies follow what you do or link your visits together.",
        Category::History => "Records kept on this computer of what you opened, copied or did.",
        Category::Location => "Finding out where you are.",
        Category::Network => "What your computer reveals to, or receives from, the networks it is on.",
        Category::Sharing => "Devices, screens and data shared with apps or other devices.",
        Category::Sandbox => "What sandboxed apps are allowed to reach.",
        Category::Updates => "Apps contacting their makers to look for new versions.",
        Category::Security => "Locks and protections that keep others out of your data.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_and_no_long_dashes() {
        assert!(setting("unit.avahi").unwrap().contains("Avahi"));
        assert!(setting("brave@Default.p3a").unwrap().contains("P3A"));
        assert!(setting("flatpak.org.x.shared.network").unwrap().contains("Network access"));
        assert!(setting("flatpak.org.x.filesystems.xdg-download").unwrap().contains("sandbox"));
        assert!(setting("firefox@abc.trr").unwrap().contains("DNS over HTTPS"));
        for (_, t) in SETTINGS {
            assert!(!t.chars().any(|c| ('\u{2012}'..='\u{2015}').contains(&c)), "{t}");
        }
    }
}
