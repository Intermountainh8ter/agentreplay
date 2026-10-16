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
    /// Replay a trajectory against its own recorded responses
    /// (reproducibility smoke test). Prints the ReplayReport.
    ///
    /// Live-endpoint replay is a follow-up; this subcommand runs the
    /// pre-reg §2 byte-identical replay against the built-in
    /// `RecordedEndpoint` stub, which proves the replay plumbing works
    /// and that the trajectory is internally consistent (one captured
    /// LlmResponse per LlmRequest).
    Replay {
        trajectory: std::path::PathBuf,
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
        Cmd::Replay { trajectory } => {
            let t: Trajectory = load_json(&trajectory)?;
            let mut ep = RecordedEndpoint::from_trajectory(&t);
            let report = replay(&t, &mut ep);
            let reproducible = report.reproducible;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !reproducible {
                std::process::exit(1);
            }
        }
        Cmd::Version => println!("agentreplay {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
