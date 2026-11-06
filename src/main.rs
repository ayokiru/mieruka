mod checks;
mod context;
mod discover;
mod markdown;
mod model;
mod reads;
mod server;
mod util;
mod workspace;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mieruka", version, about = "See what your coding agent sees")]
struct Cli {
    /// Project folders to open (defaults to the current folder)
    paths: Vec<PathBuf>,
    /// Port for the local web UI
    #[arg(long, default_value_t = 4318)]
    port: u16,
    /// Do not open the browser
    #[arg(long)]
    no_open: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };
    let roots = paths
        .iter()
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("cannot open {}", p.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    server::serve(roots, cli.port, !cli.no_open).await
}
