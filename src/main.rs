mod api;
mod mounts;
mod nodes;
mod view;

use std::process::ExitCode;

use clap::Parser;
use rustix::fs::Mode;
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

#[derive(Parser)]
#[command(about = "Serves FUSE project views configured through JSON-RPC")]
struct Args {
    #[arg(long, value_name = "HOST:PORT")]
    listen: String,
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
    match api::serve(&args.listen) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
