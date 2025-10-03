use std::env;
use std::path::{Path, PathBuf};

static EXPECTED_PAGES: &[&str] = &[
    "git-perf.1",
    "git-perf-measure.1",
    "git-perf-add.1",
    "git-perf-push.1",
    "git-perf-pull.1",
    "git-perf-report.1",
    "git-perf-audit.1",
    "git-perf-bump-epoch.1",
    "git-perf-remove.1",
    "git-perf-prune.1",
];

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

fn find_man_dir() -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .find(|p| p.file_name().unwrap_or_default() == "target")
        .unwrap();

    // Try multiple possible paths for the man directory
    let possible_paths = [
        // Standard path: workspace_root/man/man1
        target_dir.parent().unwrap().join("man").join("man1"),
        // Alternative: target/man/man1 (if build script writes there)
        target_dir.join("man").join("man1"),
        // Alternative: relative to current working directory
        std::env::current_dir().unwrap().join("man").join("man1"),
        // Alternative: relative to workspace root from current dir
        std::env::current_dir().unwrap().join("../man").join("man1"),
    ];

    // Find the first path that exists and contains manpages
    for path in &possible_paths {
        if path.exists() {
            // Check if it contains at least one manpage
            if let Ok(entries) = std::fs::read_dir(path) {
                let mut has_manpages = false;
                for entry in entries.flatten() {
                    if entry.path().extension().is_some_and(|ext| ext == "1") {
                        has_manpages = true;
                        break;
                    }
                }
                if has_manpages {
                    return path.clone();
                }
            }
        }
    }

    // If none found, return the first one (will be used for error reporting)
    possible_paths[0].clone()
}

fn find_docs_dir() -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .find(|p| p.file_name().unwrap_or_default() == "target")
        .unwrap();

    // Try multiple possible paths for the docs directory
    let possible_paths = [
        // Standard path: workspace_root/docs
        target_dir.parent().unwrap().join("docs"),
        // Alternative: relative to current working directory
        std::env::current_dir().unwrap().join("docs"),
        // Alternative: relative to workspace root from current dir
        std::env::current_dir().unwrap().join("../docs"),
    ];

    // Find the first path that exists and contains the markdown file
    for path in &possible_paths {
        if path.exists() && path.join("manpage.md").exists() {
            return path.clone();
        }
    }

    // If none found, return the first one (will be used for error reporting)
    possible_paths[0].clone()
}

#[test]
fn test_manpage_generation() {
    // Get the target directory where manpages should be generated
    let man_dir = find_man_dir();

    // Check if the man directory exists at all
    if !man_dir.exists() {
        panic!(
            "Man directory does not exist: {}. This suggests the build script did not run or generated files in a different location. Please ensure 'cargo build' is run before 'cargo test'.",
            man_dir.display()
        );
    }

    // Check that each expected manpage exists
    for page in EXPECTED_PAGES.iter() {
        let page_path = &man_dir.join(page);
        assert!(
            page_path.exists(),
            "Missing man page: {}",
            page_path.display()
        );

        // Basic content validation - check that the file is not empty
        let content = std::fs::read_to_string(page_path)
            .unwrap_or_else(|_| panic!("Failed to read manpage: {}", page_path.display()));

        assert!(
            !content.trim().is_empty(),
            "Manpage {} is empty",
            page_path.display()
        );

        // Check for basic manpage structure (should contain .TH header)
        assert!(
            content.contains(".TH"),
            "Manpage {} does not contain .TH header",
            page_path.display()
        );

        // Check for command name in the manpage
        // For subcommands, the .TH header uses the short name (e.g., "measure" not "git-perf-measure")
        let command_name = if page.starts_with("git-perf-") {
            page.replace("git-perf-", "").replace(".1", "")
        } else {
            page.replace(".1", "")
        };

        // Check that the manpage contains either the full command name or the short name
        let full_command_name = page.replace(".1", "");
        assert!(
            content.contains(&command_name) || content.contains(&full_command_name),
            "Manpage {} does not contain command name '{}' or '{}'",
            page_path.display(),
            command_name,
            full_command_name
        );
    }

    println!("All {} manpages found and validated.", EXPECTED_PAGES.len());
}

#[test]
fn test_markdown_generation() {
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
    let content = std::fs::read_to_string(&markdown_path).unwrap_or_else(|_| {
        panic!(
            "Failed to read markdown documentation: {}",
            markdown_path.display()
        )
    });

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

    println!(
        "All {} commands found in markdown documentation.",
        EXPECTED_COMMANDS.len()
    );
}

#[test]
fn test_manpage_content_validation() {
    let man_dir = find_man_dir();

    // Test main git-perf manpage for specific content
    let main_manpage = man_dir.join("git-perf.1");

    assert!(main_manpage.exists(), "Main manpage does not exist");

    let content = std::fs::read_to_string(&main_manpage).expect("Failed to read main manpage");

    // Check for essential sections
    let required_sections = [".TH", ".SH NAME", ".SH SYNOPSIS", ".SH DESCRIPTION"];
    for section in &required_sections {
        assert!(
            content.contains(section),
            "Main manpage missing required section: {}",
            section
        );
    }

    // Check for subcommand references (they appear as git\-perf\-command(1) in the main manpage)
    let subcommands = [
        "git\\-perf\\-measure(1)",
        "git\\-perf\\-add(1)",
        "git\\-perf\\-push(1)",
        "git\\-perf\\-pull(1)",
        "git\\-perf\\-report(1)",
        "git\\-perf\\-audit(1)",
    ];

    for subcommand in &subcommands {
        assert!(
            content.contains(subcommand),
            "Main manpage missing subcommand reference: {}",
            subcommand
        );
    }
}

#[test]
fn test_markdown_content_validation() {
    let docs_dir = find_docs_dir();
    let markdown_path = docs_dir.join("manpage.md");

    assert!(
        markdown_path.exists(),
        "Markdown documentation does not exist"
    );

    let content =
        std::fs::read_to_string(&markdown_path).expect("Failed to read markdown documentation");

    // Check for essential sections in clap_markdown format
    let required_sections = ["# Command-Line Help", "## `git-perf`", "**Usage:**"];
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
