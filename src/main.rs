mod agents;
mod checks;
mod cli;
mod context;
mod discover;
mod markdown;
mod model;
mod reads;
mod review;
mod server;
mod util;
mod workspace;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "mieruka",
    version,
    about = "See what your coding agent sees",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Project folders to open in the web UI (defaults to the current folder)
    paths: Vec<PathBuf>,
    /// Port for the local web UI
    #[arg(long, default_value_t = 4318)]
    port: u16,
    /// Do not open the browser
    #[arg(long)]
    no_open: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Check agent instructions for problems; exits with 1 on errors or warnings
    Check {
        /// Project folders (defaults to the current folder)
        paths: Vec<PathBuf>,
        #[arg(long, value_enum, default_value = "text")]
        format: cli::Format,
    },
    /// Show what an agent loads in a project folder
    Context {
        /// Project folder (defaults to the current folder)
        path: Option<PathBuf>,
        /// Only this agent: claude or codex
        #[arg(long)]
        agent: Option<String>,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
}

fn roots(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let default = [PathBuf::from(".")];
    let paths = if paths.is_empty() {
        &default[..]
    } else {
        paths
    };
    paths
        .iter()
        .map(|p| crate::util::canonical(p).with_context(|| format!("cannot open {}", p.display())))
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Check { paths, format }) => {
            // 1 = issues found, 2 = the check itself failed.
            let code = roots(&paths)
                .and_then(|r| cli::check(&r, format))
                .unwrap_or_else(|e| {
                    eprintln!("Error: {e:#}");
                    2
                });
            std::process::exit(code);
        }
        Some(Command::Context { path, agent, json }) => {
            let root = roots(path.as_slice())?.remove(0);
            cli::context(&root, agent.as_deref(), json)
        }
        None => server::serve(roots(&cli.paths)?, cli.port, !cli.no_open).await,
    }
}
