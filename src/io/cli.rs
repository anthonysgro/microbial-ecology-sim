// COLD PATH: CLI argument parsing. Runs once at startup.
// No external crate dependency — the argument surface is small enough for manual parsing.

use std::path::PathBuf;

use crate::io::config_error::ConfigError;

/// Parsed CLI arguments for both terminal and Bevy binaries.
#[derive(Debug, Clone, PartialEq)]
pub struct CliArgs {
    /// Path to a TOML configuration file, if `--config <path>` was provided.
    pub config_path: Option<PathBuf>,
    /// Positional seed override. Takes precedence over the seed in the TOML file.
    pub seed_override: Option<u64>,
}

/// Parse CLI arguments from `std::env::args()`.
///
/// Returns `ConfigError::CliError` on malformed input (e.g. `--config` without
/// a path, or a non-numeric positional seed).
pub fn parse_cli_args() -> Result<CliArgs, ConfigError> {
    let args: Vec<String> = std::env::args().collect();
    parse_cli_args_from(&args[1..])
}

/// Parse CLI arguments from an explicit slice (skipping the binary name).
///
/// Argument grammar (order-independent):
/// - `--config <path>` — optional, path to TOML file
/// - Positional `<seed>` — optional u64, overrides TOML seed if both present
pub fn parse_cli_args_from(args: &[String]) -> Result<CliArgs, ConfigError> {
    let mut config_path: Option<PathBuf> = None;
    let mut seed_override: Option<u64> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if arg == "--config" {
            i += 1;
            if i >= args.len() {
                return Err(ConfigError::CliError {
                    reason: "--config requires a file path argument".to_string(),
                });
            }
            config_path = Some(PathBuf::from(&args[i]));
        } else if arg.starts_with("--") {
            return Err(ConfigError::CliError {
                reason: format!("unknown flag: {arg}"),
            });
        } else {
            // Positional argument: treat as seed.
            let seed: u64 = arg.parse().map_err(|_| ConfigError::CliError {
                reason: format!("invalid seed value '{arg}': expected a non-negative integer"),
            })?;
            seed_override = Some(seed);
        }

        i += 1;
    }

    Ok(CliArgs {
        config_path,
        seed_override,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn empty_args_returns_none_none() {
        let result = parse_cli_args_from(&args(&[])).expect("should parse");
        assert_eq!(result.config_path, None);
        assert_eq!(result.seed_override, None);
    }

    #[test]
    fn config_flag_extracts_path() {
        let result =
            parse_cli_args_from(&args(&["--config", "world.toml"])).expect("should parse");
        assert_eq!(result.config_path, Some(PathBuf::from("world.toml")));
        assert_eq!(result.seed_override, None);
    }

    #[test]
    fn positional_seed_parsed() {
        let result = parse_cli_args_from(&args(&["99"])).expect("should parse");
        assert_eq!(result.config_path, None);
        assert_eq!(result.seed_override, Some(99));
    }

    #[test]
    fn config_and_seed_together() {
        let result = parse_cli_args_from(&args(&["--config", "my.toml", "123"]))
            .expect("should parse");
        assert_eq!(result.config_path, Some(PathBuf::from("my.toml")));
        assert_eq!(result.seed_override, Some(123));
    }

    #[test]
    fn seed_before_config() {
        let result = parse_cli_args_from(&args(&["42", "--config", "path.toml"]))
            .expect("should parse");
        assert_eq!(result.config_path, Some(PathBuf::from("path.toml")));
        assert_eq!(result.seed_override, Some(42));
    }

    #[test]
    fn config_without_path_errors() {
        let result = parse_cli_args_from(&args(&["--config"]));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--config requires a file path argument"));
    }

    #[test]
    fn non_numeric_seed_errors() {
        let result = parse_cli_args_from(&args(&["not_a_number"]));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid seed value"));
    }

    #[test]
    fn unknown_flag_errors() {
        let result = parse_cli_args_from(&args(&["--verbose"]));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown flag: --verbose"));
    }
}
