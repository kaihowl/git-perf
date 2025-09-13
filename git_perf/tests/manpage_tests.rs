use std::env;
use std::path::{Path, PathBuf};

static EXPECTED_COMMANDS: &[&str] = &[
    "git-perf",
    "git-perf-measure",
    "git-perf-add",
    "git-perf-push",
    "git-perf-pull",
    "git-perf-report",
    "git-perf-audit",
    "git-perf-bump-epoch",
    "git-perf-remove",
    "git-perf-prune",
];

fn find_docs_dir() -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .find(|p| p.file_name().unwrap_or_default() == "target")
        .unwrap();

    let docs_dir = target_dir.join("../../docs");

    docs_dir
}

#[test]
fn test_manpage_generation() {
    // Get the docs directory where markdown documentation should be generated
    let docs_dir = find_docs_dir();
    let markdown_path = docs_dir.join("manpage.md");

    // Check that the markdown documentation exists
    assert!(
        markdown_path.exists(),
        "Missing markdown documentation: {}",
        markdown_path.display()
    );

    // Basic content validation - check that the file is not empty
    let content = std::fs::read_to_string(&markdown_path)
        .unwrap_or_else(|_| panic!("Failed to read markdown documentation: {}", markdown_path.display()));

    assert!(
        !content.trim().is_empty(),
        "Markdown documentation {} is empty",
        markdown_path.display()
    );

    // Check that each expected command is documented in the markdown
    for command in EXPECTED_COMMANDS.iter() {
        assert!(
            content.contains(command),
            "Markdown documentation does not contain command: {}",
            command
        );
    }

    println!("All {} commands found in markdown documentation.", EXPECTED_COMMANDS.len());
}

#[test]
fn test_manpage_content_validation() {
    let docs_dir = find_docs_dir();
    let markdown_path = docs_dir.join("manpage.md");

    assert!(markdown_path.exists(), "Markdown documentation does not exist");

    let content = std::fs::read_to_string(&markdown_path).expect("Failed to read markdown documentation");

    // Check for essential sections in markdown format
    let required_sections = ["# NAME", "# SYNOPSIS", "# DESCRIPTION"];
    for section in &required_sections {
        assert!(
            content.contains(section),
            "Markdown documentation missing required section: {}",
            section
        );
    }

    // Check for subcommand references in markdown format
    let subcommands = [
        "git-perf-measure",
        "git-perf-add",
        "git-perf-push",
        "git-perf-pull",
        "git-perf-report",
        "git-perf-audit",
    ];

    for subcommand in &subcommands {
        assert!(
            content.contains(subcommand),
            "Markdown documentation missing subcommand reference: {}",
            subcommand
        );
    }
}
