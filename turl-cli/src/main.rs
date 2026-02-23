use std::process::ExitCode;

use clap::Parser;
use turl_core::{
    ProviderRoots, ThreadUri, TurlError, read_thread_raw, render_subagent_view_markdown,
    render_thread_markdown, resolve_subagent_view, resolve_thread, subagent_view_to_raw_json,
};

#[derive(Debug, Parser)]
#[command(name = "turl", version, about = "Resolve and read code-agent threads")]
struct Cli {
    /// Thread URI like amp://<session_id>, codex://<session_id>, codex://threads/<session_id>, claude://<session_id>, gemini://<session_id>, or opencode://<session_id>
    uri: String,

    /// Output raw JSON instead of markdown
    #[arg(long)]
    raw: bool,

    /// List subagents for a main thread URI
    #[arg(long)]
    list: bool,
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

    if cli.list || uri.agent_id.is_some() {
        if cli.list && uri.agent_id.is_some() {
            return Err(TurlError::InvalidMode(
                "--list cannot be used with <provider>://<main_thread_id>/<agent_id>".to_string(),
            ));
        }

        let view = resolve_subagent_view(&uri, &roots, cli.list)?;

        if cli.raw {
            let raw_json = subagent_view_to_raw_json(&view)?;
            print!("{raw_json}");
        } else {
            let markdown = render_subagent_view_markdown(&view);
            print!("{markdown}");
        }
        return Ok(());
    }

    let resolved = resolve_thread(&uri, &roots)?;

    if cli.raw {
        let content = read_thread_raw(&resolved.path)?;
        print!("{content}");
    } else {
        let markdown = render_thread_markdown(&uri, &resolved)?;
        print!("{markdown}");
    }

    Ok(())
}
