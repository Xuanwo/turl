use std::process::ExitCode;

use clap::Parser;
use turl_core::{
    ProviderRoots, ThreadUri, read_thread_raw, render_thread_markdown, resolve_thread,
};

#[derive(Debug, Parser)]
#[command(name = "turl", version, about = "Resolve and read code-agent threads")]
struct Cli {
    /// Thread URI like codex://<session_id>, codex://threads/<session_id>, or claude://<session_id>
    uri: String,

    /// Output raw JSON instead of markdown
    #[arg(long)]
    raw: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> turl_core::Result<()> {
    let roots = ProviderRoots::from_env_or_home()?;
    let uri = ThreadUri::parse(&cli.uri)?;
    let resolved = resolve_thread(&uri, &roots)?;

    for warning in &resolved.metadata.warnings {
        eprintln!("warning: {warning}");
    }

    if cli.raw {
        let content = read_thread_raw(&resolved.path)?;
        print!("{content}");
    } else {
        let markdown = render_thread_markdown(&uri, &resolved)?;
        print!("{markdown}");
    }

    Ok(())
}
