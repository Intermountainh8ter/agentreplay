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
    /// Score never-calls, always-calls and seeded random callers against
    /// tool-call-presence assertions derived from synthetic trajectories.
    Chance {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        trajectories: usize,
        #[arg(long)]
        turns: usize,
        #[arg(long)]
        call_share: f64,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Replay synthetic trajectories with the first k turns served from the
    /// record and the rest from a degenerate stub; report divergence share.
    Cutpoint {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        trajectories: usize,
        #[arg(long)]
        turns: usize,
        #[arg(long)]
        call_share: f64,
        #[arg(long, value_delimiter = ',')]
        cuts: Vec<usize>,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Score a uniform random caller's run-to-run consistency against closed
    /// forms.
    Consistency {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        tasks: usize,
        #[arg(long)]
        runs: usize,
        #[arg(long)]
        calls: usize,
        #[arg(long)]
        tools: usize,
        #[arg(long)]
        arg_values: usize,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Score empty, constant and random policies against synthetic tasks with
    /// accepting graders and unsolvable tasks.
    Emptyfloor {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        tasks: usize,
        #[arg(long)]
        accept_share: f64,
        #[arg(long)]
        unsolvable_share: f64,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Sweep the cached-step share of a synthetic replay whose uncached steps
    /// flip a tool-call decision; report byte-replay divergence rate.
    /// Perturb one recorded tool result per synthetic trajectory and count
    /// where replay flags drift, request-sensitive stub vs recorded endpoint.
    Perturb {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        trajectories: usize,
        #[arg(long)]
        turns: usize,
        #[arg(long)]
        sensitivity: f64,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    Ordersweep {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        trajectories: usize,
        #[arg(long)]
        turns: usize,
        #[arg(long)]
        flip_prob: f64,
        #[arg(long, value_delimiter = ',')]
        cache_shares: Vec<f64>,
        #[arg(long)]
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
        Cmd::Chance { seed, trajectories, turns, call_share, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::chance::run(seed, trajectories, turns, call_share);
            let mut v = serde_json::to_value(&r)?;
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay chance --seed {seed} --trajectories {trajectories} --turns {turns} --call-share {call_share} --out {}", out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Cutpoint { seed, trajectories, turns, call_share, cuts, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::cutpoint::run(seed, trajectories, turns, call_share, &cuts);
            let mut v = serde_json::to_value(&r)?;
            let cuts_s: Vec<String> = cuts.iter().map(|c| c.to_string()).collect();
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay cutpoint --seed {seed} --trajectories {trajectories} --turns {turns} --call-share {call_share} --cuts {} --out {}", cuts_s.join(","), out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Consistency { seed, tasks, runs, calls, tools, arg_values, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::consistency::run(seed, tasks, runs, calls, tools, arg_values);
            let mut v = serde_json::to_value(&r)?;
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay consistency --seed {seed} --tasks {tasks} --runs {runs} --calls {calls} --tools {tools} --arg-values {arg_values} --out {}", out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Emptyfloor { seed, tasks, accept_share, unsolvable_share, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::emptyfloor::run(seed, tasks, accept_share, unsolvable_share);
            let mut v = serde_json::to_value(&r)?;
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay emptyfloor --seed {seed} --tasks {tasks} --accept-share {accept_share} --unsolvable-share {unsolvable_share} --out {}", out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Ordersweep { seed, trajectories, turns, flip_prob, cache_shares, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::ordersweep::run(seed, trajectories, turns, flip_prob, &cache_shares);
            let mut v = serde_json::to_value(&r)?;
            let shares_s: Vec<String> = cache_shares.iter().map(|c| c.to_string()).collect();
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay ordersweep --seed {seed} --trajectories {trajectories} --turns {turns} --flip-prob {flip_prob} --cache-shares {} --out {}", shares_s.join(","), out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Perturb { seed, trajectories, turns, sensitivity, out } => {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let t0 = std::time::Instant::now();
            let r = agentreplay::perturb::run(seed, trajectories, turns, sensitivity);
            let mut v = serde_json::to_value(&r)?;
            v["provenance"] = serde_json::json!({
                "started_at": iso(started),
                "seeds": [seed],
                "command": format!("agentreplay perturb --seed {seed} --trajectories {trajectories} --turns {turns} --sensitivity {sensitivity} --out {}", out.display()),
                "hardware": "cpu",
                "wall_s": t0.elapsed().as_secs_f64(),
                "data_sources": [{"url": "synthetic: seeded generator in-repo", "license": "MIT"}],
            });
            std::fs::write(&out, serde_json::to_vec_pretty(&v)?)?;
            println!("wrote {}", out.display());
        }
        Cmd::Version => println!("agentreplay {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

fn iso(secs: u64) -> String {
    let (days, rem) = (secs / 86400, secs % 86400);
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
