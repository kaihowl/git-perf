use anyhow::Result;
use git_perf::cli;

// Main entry point
fn main() -> Result<()> {
    cli::handle_calls()
}
