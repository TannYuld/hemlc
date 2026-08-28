mod compiler;
mod core;

use clap::Parser;
use notify::Config;
use notify::Event;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::channel;

use crate::compiler::codegen::Compiler;
use crate::compiler::codegen::minify;
use crate::compiler::resolver;
use crate::core::error::CompileError;
use crate::core::error::Result;
use crate::core::lexer;
use crate::core::parser;
use crate::core::types::Document;
use crate::core::types::ExtendedDocument;

pub const MIN_JS_CORE: &str = include_str!(concat!(env!("OUT_DIR"), "/core.min.js"));
pub const HELP_MSG: &str = include_str!("./help.msg");

#[derive(Parser, Debug)]
#[command(
    name = "hemlc",
    version,
    override_help = HELP_MSG
)]
struct Cli {
    #[arg(required_unless_present = "update")]
    inputs: Vec<String>,

    #[arg(short, long)]
    out: Option<String>,

    #[arg(short, long)]
    minify: Option<usize>,

    #[arg(short, long)]
    flatten: bool,

    #[arg(long, exclusive = true)]
    update: bool,

    #[arg(short, long)]
    watch: bool,

    #[arg(short, long)]
    check: bool,

    #[arg(short, long)]
    quiet: bool,
}

fn is_component_file(source: &str) -> bool {
    let s = source.trim_start();
    if !s.starts_with('<') {
        return false;
    }

    if let Some(end_idx) = s.find('>') {
        let tag_content = &s[1..end_idx];
        let compacted: String = tag_content.chars().filter(|c| !c.is_whitespace()).collect();
        return compacted.eq_ignore_ascii_case("!doctypecomponent");
    }

    false
}

fn collect_dir_recursive(dir: &Path, base: &Path, files: &mut Vec<(PathBuf, PathBuf)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                collect_dir_recursive(&path, base, files);
            } else if path.extension().is_some_and(|ext| ext == "heml") {
                files.push((path, base.to_path_buf()));
            }
        }
    }
}

fn collect_files(inputs: &[String], quiet: bool) -> Vec<(PathBuf, PathBuf)> {
    let mut files = Vec::new();

    for input in inputs {
        let mut path = PathBuf::from(input);

        if path.extension().is_none() && !path.is_dir() {
            path.set_extension("heml");
        }

        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "heml") {
                let base = path.parent().unwrap_or(Path::new("")).to_path_buf();
                files.push((path.clone(), base));
            } else if !quiet {
                eprintln!(
                    "Input file '{}' must have a .heml extension.",
                    path.display()
                );
            }
        } else if path.is_dir() {
            collect_dir_recursive(&path, &path, &mut files);
        } else {
            if !quiet {
                eprintln!("Path not found or inaccessible: {}", input);
            }
        }
    }

    files
}

fn process_file(
    file_path: &Path,
    base_path: &Path,
    cli: &Cli,
    is_multi_file: bool,
) -> bool {
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    if is_component_file(&source) {
        return true;
    }

    let tokens = match lexer::tokenize(file_path, &source) {
        Ok(t) => t,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    let ast: Document = match parser::parse(file_path, &source, &tokens) {
        Ok(ast) => ast,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    let east: ExtendedDocument = match resolver::resolve(file_path, ast) {
        Ok(e) => e,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    if cli.check {
        if !cli.quiet {
            println!(
                "'{}' checked successfully (syntax is valid).",
                file_path.display()
            );
        }
        return true;
    }

    let compilation_result = match cli.minify {
        Some(0)  => {
            Compiler::<minify::None>::new(east).compile()
        },
        Some(1) => {
            Compiler::<minify::Js>::new(east).compile()
        },
        Some(2) | None => {
            Compiler::<minify::All>::new(east).compile()
        },
        _ => {
            Err(CompileError::plain("Invalid minify level."))
        }
    };

    let doc = match compilation_result {
        Ok(d) => d,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    let out_path = if let Some(ref out) = cli.out {
        let out_p = PathBuf::from(out);

        let is_dir_output =
            is_multi_file || out.ends_with('/') || out.ends_with('\\') || out_p.is_dir();

        if is_dir_output {
            let relative_path = if cli.flatten {
                PathBuf::from(file_path.file_name().unwrap())
            } else {
                file_path
                    .strip_prefix(base_path)
                    .unwrap_or_else(|_| Path::new(file_path.file_name().unwrap()))
                    .to_path_buf()
            };

            let full_out_path = out_p.join(relative_path).with_extension("html");
            if let Some(parent) = full_out_path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                if !cli.quiet {
                    eprintln!("Failed to create output directory {:?}: {}", parent, e);
                }
                return false;
            }

            full_out_path
        } else {
            if out_p.extension().is_some_and(|ext| ext == "html") {
                if let Some(parent) = out_p.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                out_p
            } else {
                if !cli.quiet {
                    eprintln!(
                        "Output file '{}' must have a .html extension.",
                        out_p.display()
                    );
                }
                return false;
            }
        }
    } else {
        file_path.with_extension("html")
    };

    let mut out_file = match File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            if !cli.quiet {
                eprintln!("Failed to create output file {:?}: {}", out_path, e);
            }
            return false;
        }
    };

    if let Err(e) = out_file.write_all(doc.as_bytes()) {
        if !cli.quiet {
            eprintln!("Failed to write to {:?}: {}", out_path, e);
        }
        return false;
    }

    if !cli.quiet {
        println!("Compiled '{}' -> {:?}", file_path.display(), out_path);
    }

    true
}

fn watch_files(
    cli: &Cli,
    is_multi_file: bool,
) -> notify::Result<()> {
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    for input in &cli.inputs {
        let mut path = PathBuf::from(input);
        if path.extension().is_none() && !path.is_dir() {
            path.set_extension("heml");
        }
        let watch_target = if path.is_file() {
            let parent = path.parent().unwrap_or(Path::new(""));
            if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            }
        } else {
            path.as_path()
        };

        watcher.watch(watch_target, RecursiveMode::Recursive)?;
    }

    if !cli.quiet {
        println!("Watching for changes... (Press Ctrl+C to stop)");
    }

    loop {
        match rx.recv() {
            Ok(Ok(Event { kind, paths, .. })) => {
                if !(kind.is_modify() || kind.is_create()) {
                    continue;
                }

                let mut changed_paths = std::collections::HashSet::new();
                for p in paths {
                    if p.extension().is_some_and(|ext| ext == "heml") {
                        changed_paths.insert(p);
                    }
                }

                if changed_paths.is_empty() {
                    continue;
                }

                while let Ok(Ok(Event {
                    kind: k, paths: ps, ..
                })) = rx.recv_timeout(std::time::Duration::from_millis(100))
                {
                    if k.is_modify() || k.is_create() {
                        for p in ps {
                            if p.extension().is_some_and(|ext| ext == "heml") {
                                changed_paths.insert(p);
                            }
                        }
                    }
                }

                for path in changed_paths {
                    let source = fs::read_to_string(&path).unwrap_or_default();

                    if is_component_file(&source) {
                        if !cli.quiet {
                            println!(
                                "\n[Watch] Component '{}' changed. Rebuilding all files...",
                                path.file_name().unwrap().display()
                            );
                        }

                        let fresh_files = collect_files(&cli.inputs, true);
                        let current_is_multi = fresh_files.len() > 1;

                        for (file, base) in &fresh_files {
                            process_file(file, base, cli, current_is_multi);
                        }
                    } else {
                        if !cli.quiet {
                            println!("\n[Watch] File changed: {}", path.display());
                        }

                        let mut base_path = path.parent().unwrap_or(Path::new("")).to_path_buf();
                        for input in &cli.inputs {
                            let input_path = Path::new(input);
                            let absolute_input = fs::canonicalize(input_path)
                                .unwrap_or_else(|_| input_path.to_path_buf());
                            let absolute_path =
                                fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

                            if absolute_path.starts_with(&absolute_input) {
                                base_path = if absolute_input.is_dir() {
                                    absolute_input
                                } else {
                                    absolute_input
                                        .parent()
                                        .unwrap_or(Path::new(""))
                                        .to_path_buf()
                                };
                                break;
                            }
                        }

                        process_file(&path, &base_path, cli, is_multi_file);
                    }
                }
            }
            Ok(Err(e)) => {
                if !cli.quiet {
                    eprintln!("{:?}", e);
                }
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok(())
}

fn update_self() -> ExitCode {
    println!("Checking for updates...");

    let update_result = self_update::backends::github::Update::configure()
        .repo_owner("tannyuld")
        .repo_name("hemlc")
        .bin_name("bin")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build();

    match update_result.and_then(|updater| updater.update()) {
        Ok(self_update::Status::UpToDate(version)) => {
            println!("hemlc is already up to date (v{}).", version);
            ExitCode::SUCCESS
        }
        Ok(self_update::Status::Updated(version)) => {
            println!("Successfully updated to new version (v{}).", version);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Operation aborted!\n{}", e);
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.update {
        return update_self();
    }

    let mut all_success = true;

    let files_to_process = collect_files(&cli.inputs, cli.quiet);

    if files_to_process.is_empty() {
        if !cli.quiet {
            eprintln!("No valid files found to compile.");
        }
        return ExitCode::FAILURE;
    }

    let is_multi_file = files_to_process.len() > 1;

    for (file, base) in &files_to_process {
        if !process_file(file, base, &cli, is_multi_file) {
            all_success = false;
        }
    }

    if cli.watch
        && let Err(e) = watch_files(&cli, is_multi_file)
    {
        if !cli.quiet {
            eprintln!("Failed to start watcher: {:?}", e);
        }
        return ExitCode::FAILURE;
    }

    if all_success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/* TODO:
   - style/css isn't minified at all
   - With commands like this `$ hemlc ./src/ --out ./docs/ --watch`
     Compiled './src/components/about.heml' -> "./docs/components/about.html"
     Compiled './src/components/home.heml' -> "./docs/components/home.html"
     Compiled './src/index.heml' -> "./docs/index.html"`
     components are being compiled too, even though this is not intentional.
   - Import doesn't work without optional `as` key.
   - custom tag names are case insensitive currently, make it case sensitive.
*/
