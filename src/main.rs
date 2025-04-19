use anyhow::Result;
use env_logger::Env;
use git_perf::cli;

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("warn")).init();
    cli::handle_calls()
}
