use clap::CommandFactory;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Get version from Cargo.toml
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let version: &'static str = Box::leak(version.into_boxed_str());

    // Path calculation to the workspace root's man directory
    let workspace_root = out_dir.join("../../../../");
    let man_dir = workspace_root.join("man").join("man1");

    fs::create_dir_all(&man_dir).unwrap();

    // Generate manpages for the main command and all subcommands
    let mut cmd = cli_types::Cli::command();
    cmd = cmd.version(version);
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer).unwrap();
    let main_man_path = man_dir.join("git-perf.1");
    fs::write(&main_man_path, &buffer).unwrap();

    // Generate manpages for subcommands
    let mut cmd = cli_types::Cli::command();
    cmd = cmd.version(version);
    for subcmd in cmd.get_subcommands() {
        let man = clap_mangen::Man::new(subcmd.clone());
        let mut buffer: Vec<u8> = Default::default();
        man.render(&mut buffer).unwrap();
        let subcmd_name = subcmd.get_name();
        let subcmd_man_path = man_dir.join(format!("git-perf-{subcmd_name}.1"));
        fs::write(&subcmd_man_path, &buffer).unwrap();
    }

    // Tell cargo to re-run this if the CLI definition changes
    println!("cargo:rerun-if-changed=../cli_types/src/lib.rs");

    Ok(())
}
