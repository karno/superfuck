use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = superfuck::Cli::parse();
    let (code, output) = superfuck::run(cli).await;
    if matches!(code, superfuck::ExitCode::Success) {
        let _ = io::stdout().write_all(output.as_bytes());
    } else {
        let _ = io::stderr().write_all(output.as_bytes());
    }
    ExitCode::from(code as u8)
}
