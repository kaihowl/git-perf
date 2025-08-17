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

fn find_man_dir() -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .find(|p| p.file_name().unwrap_or_default() == "target")
        .unwrap();

    let man_dir = target_dir.join("man").join("man1");

    man_dir
}

#[test]
fn test_manpage_generation() {
    // Get the target directory where manpages should be generated

    let man_dir = find_man_dir();

    // Check that each expected manpage exists
    for page in EXPECTED_PAGES.iter() {
        let page_path = &man_dir.join(page);
        assert!(
            page_path.exists(),
            "Missing man page: {}",
            page_path.display()
        );

        // Basic content validation - check that the file is not empty
        let content = std::fs::read_to_string(&page_path)
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
