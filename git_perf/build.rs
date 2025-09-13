use clap::CommandFactory;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Path calculation to the workspace root's docs directory
    let workspace_root = out_dir.join("../../../../");
    let docs_dir = workspace_root.join("docs");

    fs::create_dir_all(&docs_dir).unwrap();

    // Generate markdown for the main command
    let main_markdown = clap_markdown::help_markdown::<git_perf_cli_types::Cli>();
    
    let mut markdown_content = String::new();
    markdown_content.push_str(&main_markdown);

    // Write the combined markdown to docs/manpage.md
    let markdown_path = docs_dir.join("manpage.md");
    fs::write(&markdown_path, &markdown_content).unwrap();

    // Tell cargo to re-run this if the CLI definition changes
    println!("cargo:rerun-if-changed=../git_perf_cli_types/src/lib.rs");

    Ok(())
}
