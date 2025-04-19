use anyhow::Result;
use git_perf::cli;

fn main() -> Result<()> {
    cli::handle_calls()
}
