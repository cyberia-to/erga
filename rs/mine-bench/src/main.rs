//! Command runner for the simplified mining benchmark.
mod cli;
fn main() -> std::process::ExitCode {
    match cli::run(std::env::args().collect()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mine-bench: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
