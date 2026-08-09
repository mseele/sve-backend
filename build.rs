use std::fs;
use std::path::Path;

fn main() {
    let templates_dir = Path::new("templates");
    let compiled_dir = Path::new("templates/compiled");

    println!("cargo:rerun-if-changed=templates/");

    fs::create_dir_all(compiled_dir).expect("Failed to create templates/compiled/");

    let mut compiled = 0;

    for entry in fs::read_dir(templates_dir).expect("Failed to read templates/") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("mjml") {
            continue;
        }

        let filename = path.file_stem().unwrap().to_str().unwrap();
        let content = fs::read_to_string(&path).expect("Failed to read MJML template");

        let parsed = mrml::parse(&content).expect("Failed to parse MJML template");
        let html = parsed
            .element
            .render(&mrml::prelude::render::RenderOptions::default())
            .expect("Failed to render MJML template");

        let output_path = compiled_dir.join(format!("{filename}.html"));
        fs::write(&output_path, html).expect("Failed to write compiled HTML");

        compiled += 1;
    }

    println!("cargo:warning=Compiled {compiled} MJML templates to HTML");
}
