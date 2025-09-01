// file: src/commands/git.rs
// version: 2.1.0
// guid: be0736f7-2054-4b57-82f1-b7985d18c552

use crate::executor::Executor;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgMatches, Command};
use std::env;


/// Build the git command with comprehensive subcommands
pub fn build_command() -> Command {
    Command::new("git")
        .about("Comprehensive Git operations")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("add")
                .about("Add file contents to the index")
                .arg(Arg::new("pathspec")
                    .help("Files to add")
                    .action(clap::ArgAction::Append)
                    .default_value("."))
                .arg(Arg::new("all")
                    .short('A')
                    .long("all")
                    .help("Add all files")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("update")
                    .short('u')
                    .long("update")
                    .help("Update only modified files")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("patch")
                    .short('p')
                    .long("patch")
                    .help("Interactively choose hunks")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("commit")
                .about("Record changes to the repository")
                .arg(Arg::new("message")
                    .short('m')
                    .long("message")
                    .help("Commit message (can also be read from --args-from-file)"))
                .arg(Arg::new("amend")
                    .long("amend")
                    .help("Amend the previous commit")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("all")
                    .short('a')
                    .long("all")
                    .help("Automatically stage modified files")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("signoff")
                    .short('s')
                    .long("signoff")
                    .help("Add Signed-off-by line")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("push")
                .about("Update remote refs along with associated objects")
                .arg(Arg::new("remote")
                    .help("Remote name")
                    .default_value("origin"))
                .arg(Arg::new("branch")
                    .help("Branch name"))
                .arg(Arg::new("force")
                    .short('f')
                    .long("force")
                    .help("Force push")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("force-with-lease")
                    .long("force-with-lease")
                    .help("Force push with lease")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("set-upstream")
                    .short('u')
                    .long("set-upstream")
                    .help("Set upstream branch")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("pull")
                .about("Fetch from and integrate with another repository or branch")
                .arg(Arg::new("remote")
                    .help("Remote name")
                    .default_value("origin"))
                .arg(Arg::new("branch")
                    .help("Branch name"))
                .arg(Arg::new("rebase")
                    .short('r')
                    .long("rebase")
                    .help("Rebase instead of merge")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("no-commit")
                    .long("no-commit")
                    .help("Don't automatically commit")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("status")
                .about("Show the working tree status")
                .arg(Arg::new("short")
                    .short('s')
                    .long("short")
                    .help("Give the output in short format")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("branch")
                    .short('b')
                    .long("branch")
                    .help("Show branch information")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("porcelain")
                    .long("porcelain")
                    .help("Machine-readable output")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("branch")
                .about("List, create, or delete branches")
                .arg(Arg::new("name")
                    .help("Branch name"))
                .arg(Arg::new("delete")
                    .short('d')
                    .long("delete")
                    .help("Delete branch")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("force-delete")
                    .short('D')
                    .long("force-delete")
                    .help("Force delete branch")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("all")
                    .short('a')
                    .long("all")
                    .help("List all branches")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("remote")
                    .short('r')
                    .long("remote")
                    .help("List remote branches")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("checkout")
                .about("Switch branches or restore working tree files")
                .arg(Arg::new("branch")
                    .help("Branch or commit to checkout")
                    .required(true))
                .arg(Arg::new("create")
                    .short('b')
                    .long("create")
                    .help("Create new branch")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("force")
                    .short('f')
                    .long("force")
                    .help("Force checkout")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("merge")
                .about("Join two or more development histories together")
                .arg(Arg::new("branch")
                    .help("Branch to merge")
                    .required(true))
                .arg(Arg::new("no-ff")
                    .long("no-ff")
                    .help("Create merge commit even for fast-forward")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("squash")
                    .long("squash")
                    .help("Squash commits")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("rebase")
                .about("Reapply commits on top of another base tip")
                .arg(Arg::new("upstream")
                    .help("Upstream branch"))
                .arg(Arg::new("interactive")
                    .short('i')
                    .long("interactive")
                    .help("Interactive rebase")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("continue")
                    .long("continue")
                    .help("Continue rebase")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("abort")
                    .long("abort")
                    .help("Abort rebase")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("reset")
                .about("Reset current HEAD to the specified state")
                .arg(Arg::new("commit")
                    .help("Commit to reset to"))
                .arg(Arg::new("hard")
                    .long("hard")
                    .help("Hard reset")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("soft")
                    .long("soft")
                    .help("Soft reset")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("mixed")
                    .long("mixed")
                    .help("Mixed reset (default)")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("log")
                .about("Show commit logs")
                .arg(Arg::new("oneline")
                    .long("oneline")
                    .help("Show one line per commit")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("graph")
                    .long("graph")
                    .help("Show commit graph")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("max-count")
                    .short('n')
                    .long("max-count")
                    .help("Limit number of commits")
                    .value_name("NUMBER"))
                .arg(Arg::new("since")
                    .long("since")
                    .help("Show commits since date")
                    .value_name("DATE"))
        )
        .subcommand(
            Command::new("diff")
                .about("Show changes between commits, commit and working tree, etc")
                .arg(Arg::new("cached")
                    .long("cached")
                    .help("Show staged changes")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("name-only")
                    .long("name-only")
                    .help("Show only names of changed files")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("stat")
                    .long("stat")
                    .help("Show diffstat")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("commit1")
                    .help("First commit"))
                .arg(Arg::new("commit2")
                    .help("Second commit"))
        )
        .subcommand(
            Command::new("stash")
                .about("Stash changes in a dirty working directory")
                .subcommand(Command::new("push")
                    .about("Save current changes")
                    .arg(Arg::new("message")
                        .short('m')
                        .long("message")
                        .help("Stash message")))
                .subcommand(Command::new("pop")
                    .about("Apply and remove stash"))
                .subcommand(Command::new("list")
                    .about("List all stashes"))
                .subcommand(Command::new("drop")
                    .about("Delete a stash")
                    .arg(Arg::new("stash")
                        .help("Stash to drop")))
                .subcommand(Command::new("clear")
                    .about("Delete all stashes"))
        )
        .subcommand(
            Command::new("remote")
                .about("Manage set of tracked repositories")
                .subcommand(Command::new("add")
                    .about("Add remote")
                    .arg(Arg::new("name")
                        .help("Remote name")
                        .required(true))
                    .arg(Arg::new("url")
                        .help("Remote URL")
                        .required(true)))
                .subcommand(Command::new("remove")
                    .about("Remove remote")
                    .arg(Arg::new("name")
                        .help("Remote name")
                        .required(true)))
                .subcommand(Command::new("list")
                    .about("List remotes")
                    .arg(Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .help("Show URLs")
                        .action(clap::ArgAction::SetTrue)))
        )
        .subcommand(
            Command::new("tag")
                .about("Create, list, delete or verify tags")
                .arg(Arg::new("name")
                    .help("Tag name"))
                .arg(Arg::new("commit")
                    .help("Commit to tag"))
                .arg(Arg::new("delete")
                    .short('d')
                    .long("delete")
                    .help("Delete tag")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("list")
                    .short('l')
                    .long("list")
                    .help("List tags")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("message")
                    .short('m')
                    .long("message")
                    .help("Tag message"))
        )
        .subcommand(
            Command::new("clone")
                .about("Clone a repository into a new directory")
                .arg(Arg::new("url")
                    .help("Repository URL")
                    .required(true))
                .arg(Arg::new("directory")
                    .help("Target directory"))
                .arg(Arg::new("branch")
                    .short('b')
                    .long("branch")
                    .help("Clone specific branch")
                    .value_name("BRANCH"))
                .arg(Arg::new("depth")
                    .long("depth")
                    .help("Shallow clone with depth")
                    .value_name("DEPTH"))
        )
        .subcommand(
            Command::new("fetch")
                .about("Download objects and refs from another repository")
                .arg(Arg::new("remote")
                    .help("Remote name")
                    .default_value("origin"))
                .arg(Arg::new("all")
                    .long("all")
                    .help("Fetch all remotes")
                    .action(clap::ArgAction::SetTrue))
                .arg(Arg::new("prune")
                    .short('p')
                    .long("prune")
                    .help("Remove remote-tracking references")
                    .action(clap::ArgAction::SetTrue))
        )
        .subcommand(
            Command::new("init")
                .about("Create an empty Git repository")
                .arg(Arg::new("directory")
                    .help("Directory to initialize"))
                .arg(Arg::new("bare")
                    .long("bare")
                    .help("Create bare repository")
                    .action(clap::ArgAction::SetTrue))
        )
}

/// Execute git commands with comprehensive subcommand support
pub async fn execute(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    match matches.subcommand() {
        Some(("add", sub_matches)) => execute_add(sub_matches, executor).await,
        Some(("commit", sub_matches)) => execute_commit(sub_matches, executor).await,
        Some(("push", sub_matches)) => execute_push(sub_matches, executor).await,
        Some(("pull", sub_matches)) => execute_pull(sub_matches, executor).await,
        Some(("status", sub_matches)) => execute_status(sub_matches, executor).await,
        Some(("branch", sub_matches)) => execute_branch(sub_matches, executor).await,
        Some(("checkout", sub_matches)) => execute_checkout(sub_matches, executor).await,
        Some(("merge", sub_matches)) => execute_merge(sub_matches, executor).await,
        Some(("rebase", sub_matches)) => execute_rebase(sub_matches, executor).await,
        Some(("reset", sub_matches)) => execute_reset(sub_matches, executor).await,
        Some(("log", sub_matches)) => execute_log(sub_matches, executor).await,
        Some(("diff", sub_matches)) => execute_diff(sub_matches, executor).await,
        Some(("stash", sub_matches)) => execute_stash(sub_matches, executor).await,
        Some(("remote", sub_matches)) => execute_remote(sub_matches, executor).await,
        Some(("tag", sub_matches)) => execute_tag(sub_matches, executor).await,
        Some(("clone", sub_matches)) => execute_clone(sub_matches, executor).await,
        Some(("fetch", sub_matches)) => execute_fetch(sub_matches, executor).await,
        Some(("init", sub_matches)) => execute_init(sub_matches, executor).await,
        _ => Err(anyhow!("Unknown git subcommand")),
    }
}

/// Execute git add command
async fn execute_add(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["add".to_string()];

    if matches.get_flag("all") {
        args.push("-A".to_string());
    } else if matches.get_flag("update") {
        args.push("-u".to_string());
    } else if matches.get_flag("patch") {
        args.push("-p".to_string());
    }

    if let Some(pathspecs) = matches.get_many::<String>("pathspec") {
        for pathspec in pathspecs {
            args.push(pathspec.clone());
        }
    }

    executor.execute_secure("git", &args).await
}

/// Execute git commit command
async fn execute_commit(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["commit".to_string()];

    // Check if we have a message from command line
    let mut message_provided = false;
    if let Some(message) = matches.get_one::<String>("message") {
        args.push("-m".to_string());
        args.push(message.clone());
        message_provided = true;
    }

    // If no message provided, check for additional args from file
    if !message_provided {
        if let Ok(additional_args_str) = env::var("COPILOT_AGENT_ADDITIONAL_ARGS") {
            let additional_args: Vec<&str> = additional_args_str.lines().collect();
            if !additional_args.is_empty() {
                // Use the first line as commit message
                args.push("-m".to_string());
                args.push(additional_args[0].to_string());
                // Add any additional arguments
                for arg in additional_args.iter().skip(1) {
                    if !arg.trim().is_empty() {
                        args.push(arg.to_string());
                    }
                }
            }
        }
    }

    // If still no message, this will fail with git's normal error
    if matches.get_flag("amend") {
        args.push("--amend".to_string());
    }

    if matches.get_flag("all") {
        args.push("-a".to_string());
    }

    if matches.get_flag("signoff") {
        args.push("-s".to_string());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git push command
async fn execute_push(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["push".to_string()];

    if matches.get_flag("force") {
        args.push("-f".to_string());
    } else if matches.get_flag("force-with-lease") {
        args.push("--force-with-lease".to_string());
    }

    if matches.get_flag("set-upstream") {
        args.push("-u".to_string());
    }

    if let Some(remote) = matches.get_one::<String>("remote") {
        args.push(remote.clone());
    }

    if let Some(branch) = matches.get_one::<String>("branch") {
        args.push(branch.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git pull command
async fn execute_pull(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["pull".to_string()];

    if matches.get_flag("rebase") {
        args.push("--rebase".to_string());
    }

    if matches.get_flag("no-commit") {
        args.push("--no-commit".to_string());
    }

    if let Some(remote) = matches.get_one::<String>("remote") {
        args.push(remote.clone());
    }

    if let Some(branch) = matches.get_one::<String>("branch") {
        args.push(branch.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git status command
async fn execute_status(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["status".to_string()];

    if matches.get_flag("short") {
        args.push("-s".to_string());
    }

    if matches.get_flag("branch") {
        args.push("-b".to_string());
    }

    if matches.get_flag("porcelain") {
        args.push("--porcelain".to_string());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git branch command
async fn execute_branch(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["branch".to_string()];

    if matches.get_flag("delete") {
        args.push("-d".to_string());
    } else if matches.get_flag("force-delete") {
        args.push("-D".to_string());
    }

    if matches.get_flag("all") {
        args.push("-a".to_string());
    } else if matches.get_flag("remote") {
        args.push("-r".to_string());
    }

    if let Some(name) = matches.get_one::<String>("name") {
        args.push(name.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git checkout command
async fn execute_checkout(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["checkout".to_string()];

    if matches.get_flag("create") {
        args.push("-b".to_string());
    }

    if matches.get_flag("force") {
        args.push("-f".to_string());
    }

    if let Some(branch) = matches.get_one::<String>("branch") {
        args.push(branch.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git merge command
async fn execute_merge(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["merge".to_string()];

    if matches.get_flag("no-ff") {
        args.push("--no-ff".to_string());
    }

    if matches.get_flag("squash") {
        args.push("--squash".to_string());
    }

    if let Some(branch) = matches.get_one::<String>("branch") {
        args.push(branch.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git rebase command
async fn execute_rebase(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["rebase".to_string()];

    if matches.get_flag("interactive") {
        args.push("-i".to_string());
    } else if matches.get_flag("continue") {
        args.push("--continue".to_string());
    } else if matches.get_flag("abort") {
        args.push("--abort".to_string());
    }

    if let Some(upstream) = matches.get_one::<String>("upstream") {
        args.push(upstream.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git reset command
async fn execute_reset(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["reset".to_string()];

    if matches.get_flag("hard") {
        args.push("--hard".to_string());
    } else if matches.get_flag("soft") {
        args.push("--soft".to_string());
    } else if matches.get_flag("mixed") {
        args.push("--mixed".to_string());
    }

    if let Some(commit) = matches.get_one::<String>("commit") {
        args.push(commit.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git log command
async fn execute_log(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["log".to_string()];

    if matches.get_flag("oneline") {
        args.push("--oneline".to_string());
    }

    if matches.get_flag("graph") {
        args.push("--graph".to_string());
    }

    if let Some(max_count) = matches.get_one::<String>("max-count") {
        args.push(format!("-n{}", max_count));
    }

    if let Some(since) = matches.get_one::<String>("since") {
        args.push("--since".to_string());
        args.push(since.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git diff command
async fn execute_diff(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["diff".to_string()];

    if matches.get_flag("cached") {
        args.push("--cached".to_string());
    }

    if matches.get_flag("name-only") {
        args.push("--name-only".to_string());
    }

    if matches.get_flag("stat") {
        args.push("--stat".to_string());
    }

    if let Some(commit1) = matches.get_one::<String>("commit1") {
        args.push(commit1.clone());

        if let Some(commit2) = matches.get_one::<String>("commit2") {
            args.push(commit2.clone());
        }
    }

    executor.execute_secure("git", &args).await
}

/// Execute git stash command
async fn execute_stash(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["stash".to_string()];

    match matches.subcommand() {
        Some(("push", sub_matches)) => {
            args.push("push".to_string());
            if let Some(message) = sub_matches.get_one::<String>("message") {
                args.push("-m".to_string());
                args.push(message.clone());
            }
        }
        Some(("pop", _)) => {
            args.push("pop".to_string());
        }
        Some(("list", _)) => {
            args.push("list".to_string());
        }
        Some(("drop", sub_matches)) => {
            args.push("drop".to_string());
            if let Some(stash) = sub_matches.get_one::<String>("stash") {
                args.push(stash.clone());
            }
        }
        Some(("clear", _)) => {
            args.push("clear".to_string());
        }
        _ => {
            // Default stash behavior
        }
    }

    executor.execute_secure("git", &args).await
}

/// Execute git remote command
async fn execute_remote(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["remote".to_string()];

    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            args.push("add".to_string());
            if let Some(name) = sub_matches.get_one::<String>("name") {
                args.push(name.clone());
            }
            if let Some(url) = sub_matches.get_one::<String>("url") {
                args.push(url.clone());
            }
        }
        Some(("remove", sub_matches)) => {
            args.push("remove".to_string());
            if let Some(name) = sub_matches.get_one::<String>("name") {
                args.push(name.clone());
            }
        }
        Some(("list", sub_matches)) => {
            if sub_matches.get_flag("verbose") {
                args.push("-v".to_string());
            }
        }
        _ => {
            // Default remote list
        }
    }

    executor.execute_secure("git", &args).await
}

/// Execute git tag command
async fn execute_tag(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["tag".to_string()];

    if matches.get_flag("delete") {
        args.push("-d".to_string());
    } else if matches.get_flag("list") {
        args.push("-l".to_string());
    }

    if let Some(message) = matches.get_one::<String>("message") {
        args.push("-m".to_string());
        args.push(message.clone());
    }

    if let Some(name) = matches.get_one::<String>("name") {
        args.push(name.clone());
    }

    if let Some(commit) = matches.get_one::<String>("commit") {
        args.push(commit.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git clone command
async fn execute_clone(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["clone".to_string()];

    if let Some(branch) = matches.get_one::<String>("branch") {
        args.push("-b".to_string());
        args.push(branch.clone());
    }

    if let Some(depth) = matches.get_one::<String>("depth") {
        args.push("--depth".to_string());
        args.push(depth.clone());
    }

    if let Some(url) = matches.get_one::<String>("url") {
        args.push(url.clone());
    }

    if let Some(directory) = matches.get_one::<String>("directory") {
        args.push(directory.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git fetch command
async fn execute_fetch(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["fetch".to_string()];

    if matches.get_flag("all") {
        args.push("--all".to_string());
    }

    if matches.get_flag("prune") {
        args.push("-p".to_string());
    }

    if let Some(remote) = matches.get_one::<String>("remote") {
        args.push(remote.clone());
    }

    executor.execute_secure("git", &args).await
}

/// Execute git init command
async fn execute_init(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["init".to_string()];

    if matches.get_flag("bare") {
        args.push("--bare".to_string());
    }

    if let Some(directory) = matches.get_one::<String>("directory") {
        args.push(directory.clone());
    }

    executor.execute_secure("git", &args).await
}
