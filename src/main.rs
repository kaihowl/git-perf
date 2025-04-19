use anyhow::Result;
use git_perf::cli;

fn main() -> Result<()> {
    env_logger::init();
    cli::handle_calls()
}
