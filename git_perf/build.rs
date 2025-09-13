use clap::CommandFactory;
use std::env;
use std::fs;
use std::path::PathBuf;

const EXPECTED_PAGES: &[&str] = &[
    "git-perf.1",
    "git-perf-add.1",
    "git-perf-audit.1",
    "git-perf-bump-epoch.1",
    "git-perf-init.1",
    "git-perf-measure.1",
    "git-perf-pull.1",
    "git-perf-push.1",
    "git-perf-report.1",
    "git-perf-bump-epoch",
    "git-perf-remove",
    "git-perf-prune",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Debug output for CI troubleshooting
    println!("cargo:warning=BUILD SCRIPT DEBUG: OUT_DIR = {}", out_dir.display());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Current working directory = {}", std::env::current_dir()?.display());

    // Get version from Cargo.toml
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let version: &'static str = Box::leak(version.into_boxed_str());

    // Path calculation to the workspace root
    let workspace_root = out_dir.join("../../../../../");
    let man_dir = workspace_root.join("man").join("man1");
    let docs_dir = workspace_root.join("docs");
    
    println!("cargo:warning=BUILD SCRIPT DEBUG: Workspace root = {}", workspace_root.display());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Man dir = {}", man_dir.display());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Docs dir = {}", docs_dir.display());

    fs::create_dir_all(&man_dir).unwrap();
    fs::create_dir_all(&docs_dir).unwrap();

    // Generate manpages for the main command and all subcommands
    let mut cmd = git_perf_cli_types::Cli::command();
    cmd = cmd.version(version);
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer).unwrap();
    let main_man_path = man_dir.join("git-perf.1");
    fs::write(&main_man_path, &buffer).unwrap();

    // Generate manpages for subcommands
    let mut cmd = git_perf_cli_types::Cli::command();
    cmd = cmd.version(version);
    for subcmd in cmd.get_subcommands() {
        let man = clap_mangen::Man::new(subcmd.clone());
        let mut buffer: Vec<u8> = Default::default();
        man.render(&mut buffer).unwrap();
        let subcmd_name = subcmd.get_name();
        let subcmd_man_path = man_dir.join(format!("git-perf-{subcmd_name}.1"));
        fs::write(&subcmd_man_path, &buffer).unwrap();
    }

    // Generate markdown documentation
    let main_markdown = clap_markdown::help_markdown::<git_perf_cli_types::Cli>();
    let markdown_path = docs_dir.join("manpage.md");
    fs::write(&markdown_path, &main_markdown).unwrap();

    // Debug output after file generation
    println!("cargo:warning=BUILD SCRIPT DEBUG: Generated {} manpages", EXPECTED_PAGES.len());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Generated markdown at {}", markdown_path.display());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Man directory exists after generation: {}", man_dir.exists());
    println!("cargo:warning=BUILD SCRIPT DEBUG: Docs directory exists after generation: {}", docs_dir.exists());

    // Tell cargo to re-run this if the CLI definition changes
    println!("cargo:rerun-if-changed=../git_perf_cli_types/src/lib.rs");

    Ok(())
}
