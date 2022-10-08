use clap::{Args, Parser, Subcommand};
use std::{
    fmt::format,
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct CliMeasurement {
    /// Name of the measurement
    #[arg(short = 'm', long = "measurement")]
    name: String,

    /// Key-value pairs separated by '='
    #[arg(short, long, value_parser=parse_key_value)]
    key_value: Vec<(String, String)>,
}

#[derive(Subcommand)]
enum Commands {
    /// Measure the runtime of the supplied command
    Measure {
        /// Repetitions
        #[arg(short = 'n', long, default_value = "1")]
        repetitions: i32,

        /// Command to measure
        command: Vec<String>,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Add single measurement
    Add {
        // TODO(kaihowl) this is missing float values
        /// Measured value to be added
        value: i32,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Publish performance results to remote
    Push {},

    /// Pull performance results from remote
    Pull {},

    /// Create an HTML performance report
    Report {
        /// HTML output file
        #[arg(short, long, default_value = "output.html")]
        output: PathBuf,

        // TODO(kaihowl) No check for spaces, etc... Same applies to KV parsing method.
        /// Create individual traces in the graph by grouping with the value of this selector
        #[arg(short, long)]
        separate_by: Option<String>,

        /// Limit the number of previous commits considered
        #[arg(short = 'n', long, default_value = "40")]
        max_count: i32,

        // TODO(kaihowl) No check for spaces...
        /// What to group the measurements by
        #[arg(short, long, default_value = "commit")]
        group_by: String,
    },
}

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value: no '=' found in '{}'", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Measure {
            repetitions,
            command,
            measurement,
        } => {
            println!(
                "Measurement: {}, Repetitions: {}, command: {:?}, key-values: {:?}",
                measurement.name, repetitions, command, measurement.key_value
            );
        }
        Commands::Add { value, measurement } => {
            println!(
                "Measurement: {}, value: {}, key-values: {:?}",
                measurement.name, value, measurement.key_value
            );
        }
        Commands::Push {} => todo!(),
        Commands::Pull {} => todo!(),
        Commands::Report {
            output,
            separate_by,
            max_count,
            group_by,
        } => todo!(),
    }
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
