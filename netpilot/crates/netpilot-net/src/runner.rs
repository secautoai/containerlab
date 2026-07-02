//! Command execution abstraction so plumbing logic is testable without root.

use async_trait::async_trait;
use std::process::Output;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("command failed: {cmd}: {stderr}")]
    CommandFailed { cmd: String, stderr: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NetError>;

/// Executes external commands (`ip`, `tc`, `nft`...).
#[async_trait]
pub trait Runner: Send + Sync {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output>;

    /// Run and fail on non-zero exit.
    async fn run_ok(&self, program: &str, args: &[&str]) -> Result<()> {
        let out = self.run(program, args).await?;
        if out.status.success() {
            Ok(())
        } else {
            Err(NetError::CommandFailed {
                cmd: format!("{program} {}", args.join(" ")),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            })
        }
    }
}

/// Real runner using tokio::process.
#[derive(Debug, Default, Clone)]
pub struct SystemRunner;

#[async_trait]
impl Runner for SystemRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        Ok(tokio::process::Command::new(program)
            .args(args)
            .output()
            .await?)
    }
}

/// Records invocations for tests; always succeeds.
#[derive(Debug, Default)]
pub struct RecordingRunner {
    pub calls: std::sync::Mutex<Vec<String>>,
}

#[async_trait]
impl Runner for RecordingRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(0);
        #[cfg(not(unix))]
        let status = unimplemented!("RecordingRunner is unix-only");
        Ok(Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}
