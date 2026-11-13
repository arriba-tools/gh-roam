mod api;
mod cli;
mod manifest;
mod project;
mod prompt;
mod roam;
mod sync;

use anyhow::Result;
use clap::Parser;

use api::Client;
use cli::Cli;

fn main() -> Result<()> {
    // Exit quietly when stdout closes early, as when piped into head, instead of panicking.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();
    roam::run(&Client::new()?, cli)
}
