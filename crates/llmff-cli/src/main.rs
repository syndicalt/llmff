use clap::Parser;
use llmff_core::error::LlmffError;

mod commands;

#[tokio::main]
async fn main() {
    let cli = commands::Cli::parse();
    let code = tokio::select! {
        result = commands::run(cli) => match result {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("Error: {error:#}");
                exit_code(&error)
            }
        },
        signal = interrupt_signal() => {
            if let Err(error) = signal {
                eprintln!("Error: failed to listen for interrupt signal: {error}");
                1
            } else {
                eprintln!("Error: interrupted");
                130
            }
        }
    };
    if code != 0 {
        std::process::exit(code);
    }
}

async fn interrupt_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = interrupt.recv() => Ok(()),
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

fn exit_code(error: &anyhow::Error) -> i32 {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<LlmffError>() {
            return llmff_exit_code(error);
        }
    }

    let message = error.to_string();
    if is_cli_usage_error(&message) {
        2
    } else if message.starts_with("one or more batch items failed") {
        20
    } else {
        1
    }
}

fn llmff_exit_code(error: &LlmffError) -> i32 {
    match error {
        LlmffError::ManifestParse(_)
        | LlmffError::GraphValidation(_)
        | LlmffError::UnknownStage(_)
        | LlmffError::Config(_) => 10,
        LlmffError::StageExecution { message, .. }
            if message == "stage timed out" || message.starts_with("http tool ") =>
        {
            21
        }
        LlmffError::StageExecution { .. } => 20,
        LlmffError::Backend(_) => 21,
        LlmffError::Io(_) | LlmffError::Json(_) => 22,
        LlmffError::NotImplemented(_) => 30,
    }
}

fn is_cli_usage_error(message: &str) -> bool {
    [
        "provide either manifest or --graph",
        "stream-stage cannot write to stdout",
        "events cannot stream to stdout",
        "max-concurrency must be greater than 0",
        "timeout-ms must be greater than 0",
        "retry-attempts must be greater than 0",
        "batch mode requires",
        "batch mode output paths cannot contain parent directory components",
        "expected alias=value",
        "expected non-empty alias",
        "expected non-empty value",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}
