// file: src/commands/uutils.rs
// version: 1.0.0
// guid: 22b67d94-f0e5-4823-8a59-3d7c8b4f6a2e

use crate::executor::Executor;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgMatches, Command};
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::env;
use tracing::{debug, error, info};

/// Helper function to append additional arguments from environment variable
fn append_additional_args(mut args: Vec<String>) -> Vec<String> {
    if let Ok(additional_args_str) = env::var("COPILOT_AGENT_ADDITIONAL_ARGS") {
        let additional_args: Vec<&str> = additional_args_str.lines().collect();
        for arg in additional_args {
            if !arg.trim().is_empty() {
                args.push(arg.to_string());
            }
        }
    }
    args
}

/// Build the uutils command with comprehensive Unix utilities
pub fn build_command() -> Command {
    Command::new("uutils")
        .about("Unix utilities using uutils/coreutils")
        .subcommand(
            Command::new("find")
                .about("Find files and directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to find command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("ls")
                .about("List directory contents (enhanced ls)")
                .arg(Arg::new("args")
                    .help("Arguments to pass to ls command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("cat")
                .about("Display file contents")
                .arg(Arg::new("args")
                    .help("Arguments to pass to cat command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("grep")
                .about("Search text patterns")
                .arg(Arg::new("args")
                    .help("Arguments to pass to grep command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("sort")
                .about("Sort lines of text")
                .arg(Arg::new("args")
                    .help("Arguments to pass to sort command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("wc")
                .about("Word, line, character, and byte count")
                .arg(Arg::new("args")
                    .help("Arguments to pass to wc command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("du")
                .about("Display directory space usage")
                .arg(Arg::new("args")
                    .help("Arguments to pass to du command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("df")
                .about("Display filesystem disk space usage")
                .arg(Arg::new("args")
                    .help("Arguments to pass to df command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("cp")
                .about("Copy files or directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to cp command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("mv")
                .about("Move/rename files or directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to mv command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("rm")
                .about("Remove files or directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to rm command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("mkdir")
                .about("Create directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to mkdir command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("rmdir")
                .about("Remove empty directories")
                .arg(Arg::new("args")
                    .help("Arguments to pass to rmdir command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("chmod")
                .about("Change file permissions")
                .arg(Arg::new("args")
                    .help("Arguments to pass to chmod command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("chown")
                .about("Change file ownership")
                .arg(Arg::new("args")
                    .help("Arguments to pass to chown command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("ln")
                .about("Create links between files")
                .arg(Arg::new("args")
                    .help("Arguments to pass to ln command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("stat")
                .about("Display file or filesystem status")
                .arg(Arg::new("args")
                    .help("Arguments to pass to stat command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("pwd")
                .about("Print working directory")
                .arg(Arg::new("args")
                    .help("Arguments to pass to pwd command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("who")
                .about("Show who is logged on")
                .arg(Arg::new("args")
                    .help("Arguments to pass to who command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("uname")
                .about("Show system information")
                .arg(Arg::new("args")
                    .help("Arguments to pass to uname command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("head")
                .about("Show first lines of file")
                .arg(Arg::new("args")
                    .help("Arguments to pass to head command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("tail")
                .about("Show last lines of file")
                .arg(Arg::new("args")
                    .help("Arguments to pass to tail command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("diff")
                .about("Compare files line by line")
                .arg(Arg::new("args")
                    .help("Arguments to pass to diff command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("uniq")
                .about("Extract and list unique lines")
                .arg(Arg::new("args")
                    .help("Arguments to pass to uniq command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("cut")
                .about("Cut out selected portions of each line")
                .arg(Arg::new("args")
                    .help("Arguments to pass to cut command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("tr")
                .about("Translate or delete characters")
                .arg(Arg::new("args")
                    .help("Arguments to pass to tr command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("yes")
                .about("Output a string repeatedly until killed")
                .arg(Arg::new("args")
                    .help("Arguments to pass to yes command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("date")
                .about("Display or set the system date")
                .arg(Arg::new("args")
                    .help("Arguments to pass to date command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("sleep")
                .about("Delay for a specified amount of time")
                .arg(Arg::new("args")
                    .help("Arguments to pass to sleep command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("echo")
                .about("Print text to standard output")
                .arg(Arg::new("args")
                    .help("Arguments to pass to echo command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
        .subcommand(
            Command::new("printf")
                .about("Print text with formatting")
                .arg(Arg::new("args")
                    .help("Arguments to pass to printf command")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true))
        )
}

pub async fn execute(matches: &ArgMatches, _executor: &Executor) -> Result<()> {
    info!("Executing uutils command");

    match matches.subcommand() {
        Some((command, sub_matches)) => {
            let args = sub_matches
                .get_many::<String>("args")
                .map(|values| values.map(|s| s.to_string()).collect())
                .unwrap_or_default();

            execute_uutil(command, args).await
        }
        _ => {
            error!("No uutils subcommand specified");
            Err(anyhow!("No uutils subcommand specified"))
        }
    }
}

/// Execute a uutils command with the given arguments
async fn execute_uutil(command: &str, mut args: Vec<String>) -> Result<()> {
    // Append additional arguments from environment variable
    args = append_additional_args(args);

    debug!("Executing uutil command: {} with args: {:?}", command, args);

    // Try to use the uutils multicall binary first
    let mut cmd = StdCommand::new("coreutils");
    cmd.arg(command);
    cmd.args(&args);
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    cmd.stdin(Stdio::inherit());

    let output = cmd.output().or_else(|_| {
        // Fallback to individual uutils command if multicall binary is not available
        debug!("Multicall binary not found, trying individual command: uu_{}", command);
        let mut fallback_cmd = StdCommand::new(format!("uu_{}", command));
        fallback_cmd.args(&args);
        fallback_cmd.stdout(Stdio::inherit());
        fallback_cmd.stderr(Stdio::inherit());
        fallback_cmd.stdin(Stdio::inherit());
        fallback_cmd.output()
    }).or_else(|_| {
        // Final fallback to system command if uutils is not available
        debug!("uutils not found, falling back to system command: {}", command);
        let mut system_cmd = StdCommand::new(command);
        system_cmd.args(&args);
        system_cmd.stdout(Stdio::inherit());
        system_cmd.stderr(Stdio::inherit());
        system_cmd.stdin(Stdio::inherit());
        system_cmd.output()
    })?;

    if output.status.success() {
        info!("Command {} completed successfully", command);
        Ok(())
    } else {
        let error_msg = format!(
            "Command {} failed with exit code: {:?}",
            command,
            output.status.code()
        );
        error!("{}", error_msg);
        Err(anyhow!(error_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uutils_build_command() {
        let command = build_command();
        assert_eq!(command.get_name(), "uutils");

        // Test that find subcommand exists
        let find_cmd = command.find_subcommand("find");
        assert!(find_cmd.is_some());

        // Test that ls subcommand exists
        let ls_cmd = command.find_subcommand("ls");
        assert!(ls_cmd.is_some());
    }
}
