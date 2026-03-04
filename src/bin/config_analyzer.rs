// COLD PATH: Config analyzer binary entry point.
// Loads a TOML config, validates it, runs static analysis, and prints a report.

use std::path::PathBuf;

use microbial_ecology_sim::io::analysis::{analyze, format_report};
use microbial_ecology_sim::io::config_file::{load_bevy_config, validate_world_config};

fn parse_analyzer_args() -> Result<PathBuf, anyhow::Error> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;

    while i < args.len() {
        if args[i] == "--config" {
            i += 1;
            if i >= args.len() {
                eprintln!("Usage: config-analyzer --config <path>");
                std::process::exit(1);
            }
            return Ok(PathBuf::from(&args[i]));
        }
        i += 1;
    }

    eprintln!("Usage: config-analyzer --config <path>");
    std::process::exit(1);
}

fn main() -> anyhow::Result<()> {
    let config_path = parse_analyzer_args()?;
    let bevy_config = load_bevy_config(&config_path)?;
    let config = bevy_config.world;
    validate_world_config(&config)?;
    let report = analyze(&config);
    let output = format_report(&report);
    print!("{output}");
    Ok(())
}
