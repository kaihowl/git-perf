use clap::CommandFactory;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:warning=Starting build script");
    println!(
        "cargo:warning=CARGO_BIN_NAME: {:?}",
        env::var("CARGO_BIN_NAME")
    );
    println!(
        "cargo:warning=CARGO_PKG_NAME: {:?}",
        env::var("CARGO_PKG_NAME")
    );
    println!(
        "cargo:warning=CARGO_CRATE_NAME: {:?}",
        env::var("CARGO_CRATE_NAME")
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Get version from Cargo.toml
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let version: &'static str = Box::leak(version.into_boxed_str());
    println!("cargo:warning=Version: {:?}", version);

    // Path calculation to the workspace root's man directory
    let workspace_root = out_dir.join("../../../../");
    let man_dir = workspace_root.join("man").join("man1");

    fs::create_dir_all(&man_dir).unwrap();

    println!("cargo:warning=Generating man pages in {:?}", man_dir);

    // Generate manpages for the main command and all subcommands
    let mut cmd = cli_types::Cli::command();
    cmd = cmd.version(version);
    println!("cargo:warning=Generated command structure");
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer).unwrap();
    let main_man_path = man_dir.join("git-perf.1");
    fs::write(&main_man_path, &buffer).unwrap();
    println!(
        "cargo:warning=Generated main man page at {:?}",
        main_man_path
    );

    // Generate manpages for subcommands
    let mut cmd = cli_types::Cli::command();
    cmd = cmd.version(version);
    for subcmd in cmd.get_subcommands() {
        println!("cargo:warning=Processing subcommand: {}", subcmd.get_name());
        let man = clap_mangen::Man::new(subcmd.clone());
        let mut buffer: Vec<u8> = Default::default();
        man.render(&mut buffer).unwrap();
        let subcmd_name = subcmd.get_name();
        let subcmd_man_path = man_dir.join(format!("git-perf-{}.1", subcmd_name));
        fs::write(&subcmd_man_path, &buffer).unwrap();
        println!(
            "cargo:warning=Generated subcommand man page at {:?}",
            subcmd_man_path
        );
    }

    // // Determine the installation directory for man pages
    // let install_root = if let Ok(root) = env::var("CARGO_INSTALL_ROOT") {
    //     println!("cargo:warning=Using CARGO_INSTALL_ROOT: {:?}", root);
    //     root
    // } else if let (Ok(destdir), Ok(prefix)) = (env::var("DESTDIR"), env::var("PREFIX")) {
    //     let combined = format!("{}/{}", destdir.trim_end_matches('/'), prefix.trim_start_matches('/'));
    //     println!("cargo:warning=Using DESTDIR + PREFIX: {:?}", combined);
    //     combined
    // } else if let Ok(prefix) = env::var("PREFIX") {
    //     println!("cargo:warning=Using PREFIX: {:?}", prefix);
    //     prefix
    // } else {
    //     let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
    //     let fallback = format!("{}/.cargo", home);
    //     println!("cargo:warning=No install root env var set, falling back to {:?}", fallback);
    //     fallback
    // };
    // println!("cargo:warning=Install root: {:?}", install_root);
    // let install_root = Path::new(&install_root);
    // let man1_dir = install_root.join("share/man/man1");
    // fs::create_dir_all(&man1_dir).unwrap();
    // println!("cargo:warning=Installing man pages to {:?}", man1_dir);

    // // Copy all generated manpages to the installation directory
    // for entry in fs::read_dir(&man_dir).unwrap() {
    //     let entry = entry.unwrap();
    //     let dest = man1_dir.join(entry.file_name());
    //     fs::copy(entry.path(), &dest).unwrap();
    //     println!("cargo:warning=Copied man page to {:?}", dest);
    // }

    // Tell cargo to re-run this if the CLI definition changes
    println!("cargo:rerun-if-changed=../cli_types/src/lib.rs");

    Ok(())
}
