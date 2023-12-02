use anyhow::Result;

mod audit;
mod cli;
mod config;
mod data;
mod git_interop;
mod measurement_retrieval;
mod measurement_storage;
mod reporting;
mod serialization;
mod stats;

fn main() -> Result<()> {
    cli::handle_calls()
}
