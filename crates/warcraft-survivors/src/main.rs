//! The `warcraft-survivors` launcher: the build stamp, handed to the survivors mode.

use benilla_app::BuildId;
use clap::Parser;

#[derive(Parser)]
#[command(about = "Warcraft Survivors — solo or native cooperative multiplayer")]
struct Args {
    /// Host a cooperative session (default: all network interfaces, TCP 8091).
    #[arg(long, num_args = 0..=1, default_missing_value = "0.0.0.0:8091", conflicts_with = "join")]
    host: Option<String>,
    /// Join a host, for example 192.168.1.20:8091.
    #[arg(long, value_name = "HOST:PORT")]
    join: Option<String>,
}

fn main() -> benilla_app::AppExit {
    let args = Args::parse();
    // Before Bevy or any worker threads start; the library also accepts these environment values.
    if let Some(address) = args.host {
        std::env::set_var("SURVIVORS_HOST", address);
    }
    if let Some(address) = args.join {
        std::env::set_var("SURVIVORS_JOIN", address);
    }
    benilla_app::run_survivors(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: env!("BENILLA_PROJECT_DIR"),
        ..Default::default()
    })
}
