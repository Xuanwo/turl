use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use xurl_core::{
    ProviderRoots, ThreadUri, XurlError, render_subagent_view_markdown,
    render_thread_head_markdown, render_thread_markdown, resolve_subagent_view, resolve_thread,
};

#[derive(Debug, Parser)]
#[command(name = "xurl", version, about = "Resolve and read code-agent threads")]
struct Cli {
    /// Thread URI like agents://codex/<session_id>, agents://claude/<session_id>, agents://pi/<session_id>/<entry_id>, or legacy forms like codex://<session_id>
    uri: String,

    /// Output frontmatter only (header mode)
    #[arg(short = 'I', long)]
    head: bool,

    /// Write output to a file instead of stdout
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    output: Option<PathBuf>,
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

fn run(cli: Cli) -> xurl_core::Result<()> {
    let Cli { uri, head, output } = cli;
    let roots = ProviderRoots::from_env_or_home()?;
    let uri = ThreadUri::parse(&uri)?;

    let output = output.as_deref();

    if head {
        let head = render_thread_head_markdown(&uri, &roots)?;
        return write_output(output, &head);
    }

    let markdown = if matches!(
        uri.provider,
        xurl_core::ProviderKind::Codex | xurl_core::ProviderKind::Claude
    ) && uri.agent_id.is_some()
    {
        let head = render_thread_head_markdown(&uri, &roots)?;
        let view = resolve_subagent_view(&uri, &roots, false)?;
        let body = render_subagent_view_markdown(&view);
        format!("{head}\n{body}")
    } else {
        let head = render_thread_head_markdown(&uri, &roots)?;
        let resolved = resolve_thread(&uri, &roots)?;
        let body = render_thread_markdown(&uri, &resolved)?;
        format!("{head}\n{body}")
    };

    write_output(output, &markdown)
}

fn write_output(path: Option<&Path>, content: &str) -> xurl_core::Result<()> {
    if let Some(path) = path {
        std::fs::write(path, content).map_err(|source| XurlError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    } else {
        print!("{content}");
    }

    Ok(())
}
