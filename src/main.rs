mod api;
mod mounts;
mod nodes;
mod state;
mod view;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use rustix::fs::Mode;
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

#[derive(Parser)]
#[command(about = "Serves FUSE project views configured through JSON-RPC")]
struct Args {
    #[arg(long, value_name = "HOST:PORT")]
    listen: String,
    #[arg(long, value_name = "DIR")]
    data_dir: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    // Entries are created with the modes the kernel already masked with the caller's umask.
    rustix::process::umask(Mode::empty());
    // Each node the kernel holds keeps an O_PATH descriptor open.
    let limit = getrlimit(Resource::Nofile);
    if let Err(err) = setrlimit(
        Resource::Nofile,
        Rlimit {
            current: limit.maximum,
            maximum: limit.maximum,
        },
    ) {
        eprintln!("raise the open file limit: {err}");
    }
    if let Err(err) = std::fs::create_dir_all(&args.data_dir) {
        eprintln!("create {}: {err}", args.data_dir.display());
        return ExitCode::FAILURE;
    }
    let stored = match state::load(&args.data_dir) {
        Ok(stored) => stored,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let mut registry = mounts::Registry::new(args.data_dir, stored);
    registry.restore();
    match api::serve(&args.listen, registry) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
