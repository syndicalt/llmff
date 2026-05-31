use clap::Parser;

mod commands;

#[tokio::main]
async fn main() {
    let cli = commands::Cli::parse();
    let interrupt_context = commands::interrupt_context(&cli);
    let code = match InterruptSignal::new() {
        Ok(mut interrupt_signal) => {
            tokio::select! {
                result = commands::run(cli) => match result {
                    Ok(()) => 0,
                    Err(error) => {
                        eprintln!("Error: {error:#}");
                        commands::exit_code(&error)
                    }
                },
                signal = interrupt_signal.recv() => {
                    if let Err(error) = signal {
                        eprintln!("Error: failed to listen for interrupt signal: {error}");
                        1
                    } else {
                        if let Some(context) = interrupt_context.as_ref() {
                            if let Err(error) = commands::write_interrupted_run_result(context) {
                                eprintln!("Error: failed to write interrupted run result: {error:#}");
                            }
                        }
                        eprintln!("Error: interrupted");
                        130
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("Error: failed to listen for interrupt signal: {error}");
            1
        }
    };
    if code != 0 {
        std::process::exit(code);
    }
}

#[cfg(unix)]
struct InterruptSignal {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl InterruptSignal {
    fn new() -> std::io::Result<Self> {
        use tokio::signal::unix::{signal, SignalKind};
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }

    async fn recv(&mut self) -> std::io::Result<()> {
        tokio::select! {
            _ = self.interrupt.recv() => Ok(()),
            _ = self.terminate.recv() => Ok(()),
        }
    }
}

#[cfg(not(unix))]
struct InterruptSignal;

#[cfg(not(unix))]
impl InterruptSignal {
    fn new() -> std::io::Result<Self> {
        Ok(Self)
    }

    async fn recv(&mut self) -> std::io::Result<()> {
        tokio::signal::ctrl_c().await
    }
}
