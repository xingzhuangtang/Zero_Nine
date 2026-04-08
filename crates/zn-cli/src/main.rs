use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use zn_host::detect_host;
use zn_types::HostKind;

#[derive(Parser, Debug)]
#[command(name = "zero-nine", version, about = "Zero_Nine orchestration engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
    },
    Brainstorm {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long, default_value_t = false)]
        resume: bool,
    },
    Run {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        goal: String,
        #[arg(long, default_value_t = false)]
        confirm_remote_finish: bool,
    },
    Status {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    Resume {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long, default_value_t = false)]
        confirm_remote_finish: bool,
    },
    Export {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { project, host } => {
            zn_loop::initialize_project(&project, detect_host(host.as_deref()))?;
            println!("Initialized Zero_Nine at {}", project.display());
        }
        Commands::Brainstorm {
            project,
            host,
            goal,
            resume,
        } => {
            let host = detect_host(host.as_deref());
            let output = if !resume && !matches!(host, HostKind::Terminal) {
                let input = goal
                    .as_deref()
                    .ok_or_else(|| anyhow!("goal or answer input is required for host-native brainstorming"))?;
                zn_loop::brainstorm_host_turn(&project, input, host)?
            } else {
                zn_loop::brainstorm(&project, goal.as_deref(), host, resume)?
            };
            println!("{}", output);
        }
        Commands::Run {
            project,
            host,
            goal,
            confirm_remote_finish,
        } => {
            let output = zn_loop::run_goal(
                &project,
                &goal,
                detect_host(host.as_deref()),
                confirm_remote_finish,
            )?;
            println!("{}", output);
        }
        Commands::Status { project } => {
            println!("{}", zn_loop::status(&project)?);
        }
        Commands::Resume {
            project,
            host,
            confirm_remote_finish,
        } => {
            println!(
                "{}",
                zn_loop::resume(&project, detect_host(host.as_deref()), confirm_remote_finish)?
            );
        }
        Commands::Export { project } => {
            println!("{}", zn_loop::export(&project)?);
        }
    }
    Ok(())
}
