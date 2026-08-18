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

use crate::compiler::resolver::{ExtendedDocument, resolve};
use crate::core::error::Result;
use crate::core::lexer::tokenize;
use crate::core::parser::parse;
use crate::core::types::CodegenStrategy;
use crate::core::types::CompilerOptions;
use crate::core::types::Document;

pub const MIN_JS_CORE: &'static str = include_str!(concat!(env!("OUT_DIR"), "/core.min.js"));
pub const HELP_MSG: &'static str = include_str!("./help.msg");

#[derive(Parser, Debug)]
#[command(
    name = "hemlc",
    version,
    override_help = HELP_MSG
)]
struct Cli {
    #[arg(required = true)]
    inputs: Vec<String>,

    #[arg(short, long)]
    out: Option<String>,

    #[arg(short, long)]
    minify: Option<usize>,

    #[arg(short, long)]
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

fn collect_files(inputs: &[String], quiet: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for input in inputs {
        let path = Path::new(input);

        if path.is_file() {
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.filter_map(std::result::Result::ok) {
                        let entry_path = entry.path();

                        let is_target_file = entry_path.is_file()
                            && entry_path.extension().map_or(false, |ext| ext == "heml");

                        if is_target_file {
                            files.push(entry_path);
                        }
                    }
                }
                Err(e) => {
                    if !quiet {
                        eprintln!("Failed to read directory '{}': {}", input, e);
                    }
                }
            }
        } else {
            if !quiet {
                eprintln!("Path not found or inaccessible: {}", input);
            }
        }
    }

    files
}

fn process_file(file_path: &Path, cli: &Cli, is_multi_file: bool, options: &CompilerOptions) -> bool {
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

    let tokens = match tokenize(file_path, &source) {
        Ok(t) => t,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    let ast: Document = match parse(file_path, &source, &tokens) {
        Ok(ast) => ast,
        Err(e) => {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return false;
        }
    };

    let east: ExtendedDocument = match resolve(file_path, ast) {
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

    let doc = match east.compile(*options) {
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
            if let Err(e) = fs::create_dir_all(&out_p) {
                if !cli.quiet {
                    eprintln!("Failed to create output directory {:?}: {}", out_p, e);
                }
                return false;
            }

            let file_name = file_path.file_name().unwrap();
            out_p.join(file_name).with_extension("html")
        } else {
            if let Some(parent) = out_p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            out_p
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

fn watch_files(cli: &Cli, is_multi_file: bool, compiler_options: &CompilerOptions) -> notify::Result<()> {
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    let file_count = cli.inputs.len();

    for input in &cli.inputs {
        let path = Path::new(input);
        watcher.watch(path, RecursiveMode::NonRecursive)?;
    }

    if !cli.quiet {
        println!("Watching for changes... (Press Ctrl+C to stop)");
    }

    for res in rx {
        match res {
            Ok(Event { kind, paths, .. }) => {
                if kind.is_modify() || kind.is_create() {
                    for path in paths {
                        if path.extension().map_or(false, |ext| ext == "heml") {
                            let source = fs::read_to_string(&path).unwrap_or_default();

                            if is_component_file(&source) {
                                if !cli.quiet {
                                    println!(
                                        "\n[Watch] Component '{}' changed. Rebuilding all files...",
                                        path.display()
                                    );
                                }

                                let fresh_files = collect_files(&cli.inputs, true);

                                let current_is_multi = fresh_files.len() > 1;

                                for file in &fresh_files {
                                    process_file(file, cli, current_is_multi, compiler_options);
                                }
                            } else {
                                if !cli.quiet {
                                    println!(
                                        "\n[Watch] {} File(s) changed: {}",
                                        file_count,
                                        path.display()
                                    );
                                }
                                process_file(&path, cli, is_multi_file, compiler_options);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if !cli.quiet {
                    eprintln!("Watch error: {:?}", e);
                }
            }
        }
    }

    Ok(())
}

fn update_self() -> ExitCode {
    let update = || -> std::result::Result<bool, Box<dyn std::error::Error>> {
        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner("tannyuld")
            .repo_name("hemlc")
            .build()?
            .fetch()?;

        if releases.is_empty() {
            return Err("No releases found on GitHub.".into());
        }

        let latest_release = &releases[0];
        let current_version = env!("CARGO_PKG_VERSION");

        let is_newer =
            self_update::version::bump_is_greater(current_version, &latest_release.version)
                .unwrap_or(false);

        if !is_newer {
            return Ok(false);
        }

        let asset = releases[0]
            .asset_for(&self_update::get_target(), None)
            .unwrap();

        let tmp_dir = tempfile::Builder::new()
            .prefix("hemlc_update")
            .tempdir_in(::std::env::current_dir()?)?;

        let tmp_tarball_path = tmp_dir.path().join(&asset.name);
        let tmp_tarball = ::std::fs::File::create(&tmp_tarball_path)?;

        self_update::Download::from_url(&asset.download_url)
            .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
            .download_to(&tmp_tarball)?;

        let bin_name = std::path::PathBuf::from("bin");
        self_update::Extract::from_source(&tmp_tarball_path)
            .archive(self_update::ArchiveKind::Tar(Some(
                self_update::Compression::Gz,
            )))
            .extract_file(&tmp_dir.path(), &bin_name)?;

        let new_exe = tmp_dir.path().join(bin_name);
        self_replace::self_replace(new_exe)?;

        Ok(true)
    };

    match update() {
        Ok(true) => {
            println!("Successfully updated to new version.");
            ExitCode::SUCCESS   
        },
        Ok(false) => {
            println!("hemlc is already up to date (v{}).", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        },
        Err(e) => {
            eprintln!("Failed to update: {}", e);
            ExitCode::FAILURE
        },
    }
}

fn get_compiler_options_from_cli(cli: &Cli) -> Result<CompilerOptions> {
    let mut compiler_options = CompilerOptions::default();
    if let Some(minfy_elevation) = cli.minify {
        compiler_options.codegen_strategy = CodegenStrategy::try_from(minfy_elevation)?;
    }
    Ok(compiler_options)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.update {
        return update_self();
    }
    
    let mut all_success = true;
    
    let files_to_process = collect_files(&cli.inputs, cli.quiet);
    let compiler_options = match get_compiler_options_from_cli(&cli) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        },
    };    

    if files_to_process.is_empty() {
        if !cli.quiet {
            eprintln!("No valid files found to compile.");
        }
        return ExitCode::FAILURE;
    }

    let is_multi_file = files_to_process.len() > 1;

    for file in &files_to_process {
        if !process_file(file, &cli, is_multi_file, &compiler_options) {
            all_success = false;
        }
    }

    if cli.watch {
        if let Err(e) = watch_files(&cli, is_multi_file, &compiler_options) {
            if !cli.quiet {
                eprintln!("Failed to start watcher: {:?}", e);
            }
            return ExitCode::FAILURE;
        }
    }

    if all_success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}


/*  TODO: Make this changes happen!!!
    - Let the user choose to minfy or not.
    - Fix `hemlc --update` needs at least one parameter like this `helmc --update something`.
    - Change the update fail message.
    - Add progressbar to different update phases.
    - Improve --watch command:
        - It should start to listen new created files inside a folder while already watching.
        - It should auto compile everything when any component is changed.
    - Improve heml component structure:
        - Add a `<meta>` (name subject to change) to being able to put style and script tags, or not (this one is subject to decide weather or not to implement...)
    - Improve heml file structure:
        - Force for `.heml` file format by compiler level.
*/
