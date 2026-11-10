//! Compiles the icons in icons/ into a GResource that is built into the binary, so the
//! app's own icons show on every system, whatever icon theme is installed (or none).

use std::fmt::Write;
use std::path::Path;

fn main() {
    let dir = Path::new("icons");
    println!("cargo:rerun-if-changed=icons");
    let mut files = Vec::new();
    for sub in ["scalable/actions", "scalable/apps"] {
        let mut names: Vec<String> = std::fs::read_dir(dir.join(sub))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".svg"))
            .collect();
        names.sort();
        files.extend(names.into_iter().map(|n| format!("{sub}/{n}")));
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"/guru/freberg/PrivacyPanel/icons\">\n");
    for f in &files {
        writeln!(xml, "    <file preprocess=\"xml-stripblanks\">{f}</file>").unwrap();
    }
    xml.push_str("  </gresource>\n</gresources>\n");
    let out = std::env::var("OUT_DIR").unwrap();
    let xml_path = Path::new(&out).join("icons.gresource.xml");
    std::fs::write(&xml_path, xml).unwrap();
    glib_build_tools::compile_resources(&[dir], xml_path.to_str().unwrap(), "icons.gresource");
}
