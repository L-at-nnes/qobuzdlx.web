use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use futures_util::future::BoxFuture;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::registry::now;
use super::{Event, Extension, Failure, Info, Source};

/// How much of what a program wrote to its error output is kept in the log.
const STDERR_KEPT: usize = 500;

/// An extension that is a program: it is started for each event and gets the event as one line of
/// JSON on its standard input.
///
/// It runs from its own folder with an empty environment but `PATH` (and `SYSTEMROOT` on Windows),
/// plus `QOBUZDLX_EVENT` (the name of the event) and `QOBUZDLX_EXTENSION_DIR`. The application's
/// own variables, the access password among them, are not passed on. A program that exits with
/// anything but 0, or that is still running after the timeout, is reported as failed.
#[derive(Debug, Clone)]
pub struct CommandExtension {
    info: Info,
    program: PathBuf,
    args: Vec<String>,
    directory: PathBuf,
    timeout: Duration,
}

impl CommandExtension {
    /// `command` is the program and its arguments, as in a manifest.
    #[must_use]
    pub fn new(info: Info, command: &[String], directory: PathBuf, timeout: Duration) -> Self {
        let program = command.first().map_or_else(PathBuf::new, |program| {
            // Written with a separator, it is relative to the extension: otherwise the PATH decides
            if program.contains('/') || program.contains('\\') {
                directory.join(program)
            } else {
                PathBuf::from(program)
            }
        });
        Self {
            info,
            program,
            args: command.iter().skip(1).cloned().collect(),
            directory,
            timeout,
        }
    }

    async fn run(&self, event: &Event) -> Result<(), Failure> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.directory)
            .env_clear()
            .env("QOBUZDLX_EVENT", event.kind().name())
            .env("QOBUZDLX_EXTENSION_DIR", &self.directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for name in ["PATH", "SYSTEMROOT"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }

        let mut child = command.spawn().map_err(|error| {
            Failure(format!("could not start {}: {error}", show(&self.program)))
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            let mut line = event.to_json(now());
            line.push('\n');
            // A program that does not read its input may exit first: that is not a failure
            let _ = stdin.write_all(line.as_bytes()).await;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|error| Failure(format!("could not wait for the program: {error}")))?;

        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Failure(format!(
            "{} ended with {}{}",
            show(&self.program),
            output.status,
            tail(stderr.trim())
        )))
    }
}

/// The last part of an error output, ready to follow a message.
fn tail(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let skipped = text.chars().count().saturating_sub(STDERR_KEPT);
    let kept: String = text.chars().skip(skipped).collect();
    format!(": {kept}")
}

fn show(program: &Path) -> String {
    program.display().to_string()
}

impl Extension for CommandExtension {
    fn info(&self) -> Info {
        Info {
            source: Source::Command,
            ..self.info.clone()
        }
    }

    fn handle<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            // Dropping the future on timeout kills the program (`kill_on_drop`)
            tokio::time::timeout(self.timeout, self.run(event))
                .await
                .unwrap_or_else(|_| {
                    Err(Failure(format!(
                        "{} did not finish within {} seconds and was stopped",
                        show(&self.program),
                        self.timeout.as_secs()
                    )))
                })
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn extension(script: &str, timeout: Duration) -> (CommandExtension, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let info = Info {
            name: "test".into(),
            version: None,
            description: None,
            source: Source::Command,
            events: vec![],
        };
        let command = ["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()];
        (
            CommandExtension::new(info, &command, directory.path().to_path_buf(), timeout),
            directory,
        )
    }

    #[tokio::test]
    async fn the_event_arrives_on_the_standard_input_and_in_the_environment() {
        let (extension, directory) = extension(
            r#"cat > received.json; echo "$QOBUZDLX_EVENT" > name.txt; pwd > cwd.txt"#,
            Duration::from_secs(10),
        );

        extension.handle(&Event::QueueDrained).await.unwrap();

        let received = std::fs::read_to_string(directory.path().join("received.json")).unwrap();
        assert!(
            received.contains(r#""event":"queue_drained""#),
            "{received}"
        );
        let name = std::fs::read_to_string(directory.path().join("name.txt")).unwrap();
        assert_eq!(name.trim(), "queue_drained");
        // It runs from its own folder
        assert!(directory.path().join("cwd.txt").exists());
    }

    #[tokio::test]
    async fn the_secrets_of_the_application_are_not_passed_on() {
        // SAFETY-free: only reads what the test process has, and PATH is always there
        let (extension, directory) = extension(r#"env > env.txt"#, Duration::from_secs(10));

        extension.handle(&Event::QueueDrained).await.unwrap();

        let env = std::fs::read_to_string(directory.path().join("env.txt")).unwrap();
        assert!(env.contains("QOBUZDLX_EVENT=queue_drained"));
        assert!(!env.contains("ACCESS_PASSWORD"));
        assert!(!env.contains("HOME="));
    }

    #[tokio::test]
    async fn a_failing_program_is_reported_with_its_error_output() {
        let (extension, _directory) = extension("echo broken >&2; exit 3", Duration::from_secs(10));

        let failure = extension.handle(&Event::QueueDrained).await.unwrap_err();

        assert!(failure.0.contains("exit status: 3"), "{failure}");
        assert!(failure.0.contains("broken"), "{failure}");
    }

    #[tokio::test]
    async fn a_program_that_hangs_is_stopped_at_the_timeout() {
        let (extension, _directory) = extension("sleep 30", Duration::from_millis(200));

        let failure = extension.handle(&Event::QueueDrained).await.unwrap_err();

        assert!(failure.0.contains("did not finish"), "{failure}");
    }

    #[tokio::test]
    async fn a_missing_program_is_a_failure_not_a_panic() {
        let info = Info {
            name: "test".into(),
            version: None,
            description: None,
            source: Source::Command,
            events: vec![],
        };
        let extension = CommandExtension::new(
            info,
            &["./nothing-here".to_owned()],
            std::env::temp_dir(),
            Duration::from_secs(5),
        );

        assert!(extension.handle(&Event::QueueDrained).await.is_err());
    }
}
