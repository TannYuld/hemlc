use std::env;
use std::fs;
use std::path::Path;
use minify_js::{Session, TopLevelMode, minify};

fn main() {
    println!("cargo:rerun-if-changed=src/core.js");

    let js_code = fs::read("src/core.js").expect("Failed to read core.js");

    let session = Session::new();
    let mut minified = Vec::new();
    minify(&session, TopLevelMode::Global, &js_code, &mut minified)
        .expect("Failed to minify JavaScript");

    let out_dir = env::var_os("OUT_DIR").expect("Out dir cannot be found");
    let dest_path = Path::new(&out_dir).join("core.min.js");
    fs::write(&dest_path, minified).expect("Failed to write minified JS");
}