//! agentreplay CLI — scaffold.

use agentreplay::{inject, replay, step_hashes, trajectory_hash, RecordedEndpoint, Trajectory};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agentreplay",
    version,
    about = "Deterministic agent trajectory replay"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the chained content hashes of each step and the overall
    /// trajectory hash.
    Hash {
        trajectory: std::path::PathBuf,
    },
    /// Inject a replacement step at index `at`, truncate the tail, write
    /// the modified trajectory to `out`.
    Inject {
        trajectory: std::path::PathBuf,
        /// Index of the step to replace.
        #[arg(long)]
        at: usize,
        /// Replacement step as JSON.
        #[arg(long)]
        replacement: std::path::PathBuf,
        out: std::path::PathBuf,
    },
    /// Replay a trajectory. Without `--live` it replays against its own
    /// recorded responses (the built-in `RecordedEndpoint` stub), which
    /// proves the plumbing and internal consistency. With `--live` it
    /// re-sends every recorded request to the HTTP endpoint and runs the
    /// pre-reg §2 byte-identical comparison against what comes back.
    /// Exits 1 when not reproducible.
    Replay {
        trajectory: std::path::PathBuf,
        /// JSON `HttpEndpointConfig` (`url`, optional `api_key_env`, ...).
        #[arg(long)]
        live: Option<std::path::PathBuf>,
    },
    /// Record a single-turn trajectory by POSTing a request body file to
    /// a live HTTP endpoint and capturing the raw response bytes.
    Record {
        /// JSON `HttpEndpointConfig`.
        #[arg(long)]
        endpoint: std::path::PathBuf,
        /// File whose bytes are sent verbatim as the request body.
        #[arg(long)]
        request: std::path::PathBuf,
        #[arg(long)]
        run_id: String,
        out: std::path::PathBuf,
    },
    Version,
}

fn load_json<T: serde::de::DeserializeOwned>(p: &std::path::Path) -> anyhow::Result<T> {
    Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Hash { trajectory } => {
            let t: Trajectory = load_json(&trajectory)?;
            for (i, h) in step_hashes(&t).iter().enumerate() {
                println!("[{i:03}] {h}");
            }
            println!("final  {}", trajectory_hash(&t));
        }
        Cmd::Inject {
            trajectory,
            at,
            replacement,
            out,
        } => {
            let t: Trajectory = load_json(&trajectory)?;
            let s: agentreplay::Step = load_json(&replacement)?;
            let modified = inject(&t, at, s);
            std::fs::write(&out, serde_json::to_vec_pretty(&modified)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Replay { trajectory, live } => {
            let t: Trajectory = load_json(&trajectory)?;
            let report = match live {
                None => {
                    let mut ep = RecordedEndpoint::from_trajectory(&t);
                    replay(&t, &mut ep)
                }
                Some(cfg_path) => {
                    let cfg: agentreplay::http::HttpEndpointConfig = load_json(&cfg_path)?;
                    let mut ep = agentreplay::http::HttpEndpoint::new(cfg)?;
                    let report = replay(&t, &mut ep);
                    if let Some(e) = ep.take_last_error() {
                        anyhow::bail!("live endpoint error: {e}");
                    }
                    report
                }
            };
            let reproducible = report.reproducible;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !reproducible {
                std::process::exit(1);
            }
        }
        Cmd::Record {
            endpoint,
            request,
            run_id,
            out,
        } => {
            let cfg: agentreplay::http::HttpEndpointConfig = load_json(&endpoint)?;
            let mut ep = agentreplay::http::HttpEndpoint::new(cfg)?;
            let body = std::fs::read(&request)?;
            let t = agentreplay::http::record_single_turn(&mut ep, &run_id, &body)?;
            std::fs::write(&out, serde_json::to_vec_pretty(&t)?)?;
            println!("recorded {} steps; hash {}", t.steps.len(), trajectory_hash(&t));
        }
        Cmd::Version => println!("agentreplay {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
