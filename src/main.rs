use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use qobuzdlx::access::{AccessStore, Gate, Tokens};
use qobuzdlx::config::RuntimeConfig;
use qobuzdlx::download::{Manager, ManagerConfig, Tuning};
use qobuzdlx::qobuz::{Client, Restored, Sessions};
use qobuzdlx::settings::SettingsStore;
use qobuzdlx::web::{self, AppState, Shared};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long a connection to the CDN may stay silent before the transfer is given up.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// How often a saved session is tried again when Qobuz cannot be reached at start-up.
const RESTORE_ATTEMPTS: u32 = 10;
const RESTORE_DELAY: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--healthcheck") {
        return healthcheck();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("qobuzdlx=info")),
        )
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// What the container runtime runs to know whether the server is alive.
fn healthcheck() -> ExitCode {
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    match qobuzdlx::healthcheck::target(&bind) {
        Some(address) if qobuzdlx::healthcheck::is_alive(address) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

async fn run() -> anyhow::Result<()> {
    let config = RuntimeConfig::from_env()?;
    std::fs::create_dir_all(&config.config_dir)?;
    tracing::info!(version = VERSION, bind = %config.bind, "starting QobuzDLX");

    let client = Client::production();
    let settings = SettingsStore::open(&config.config_dir, &config.default_downloads_dir);
    let access = AccessStore::open(&config.config_dir, config.access_password.as_deref())?;
    if !access.is_protected() {
        tracing::warn!(
            "no access password is set: anyone who can reach this server can use the linked \
             Qobuz account. Set one in the settings, or with ACCESS_PASSWORD"
        );
    }

    // No overall timeout: a 24-bit album takes as long as it takes. A connection that goes silent
    // is given up by the transfer itself.
    let cdn = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(format!("QobuzDLX/{VERSION}"))
        .build()?;
    let downloads = Manager::start(ManagerConfig {
        client: client.clone(),
        settings: settings.clone(),
        config_dir: config.config_dir.clone(),
        http: cdn,
        stall_timeout: STALL_TIMEOUT,
        tuning: Tuning::default(),
    });

    let state = AppState::new(Shared {
        version: VERSION,
        config_dir: config.config_dir.clone(),
        cookie_secure: config.cookie_secure,
        access,
        gate: Gate::default(),
        tokens: Tokens::open(&config.config_dir),
        sessions: Sessions::new(client.clone(), &config.config_dir),
        client,
        settings,
        downloads,
    });

    restore_session(&state);

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, web::router(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("stopping");
    state.downloads.shutdown();
    Ok(())
}

/// Opens the saved Qobuz session again in the background, and forgets it if Qobuz refuses it later.
fn restore_session(state: &AppState) {
    let state = Arc::new(state.clone());

    let watcher = Arc::clone(&state);
    tokio::spawn(async move { watcher.sessions.forget_when_refused().await });

    tokio::spawn(async move {
        match state
            .sessions
            .restore(RESTORE_ATTEMPTS, RESTORE_DELAY)
            .await
        {
            Restored::Unreachable => {
                tracing::warn!("Qobuz could not be reached: sign in again when it can");
            }
            Restored::Nothing | Restored::Resumed | Restored::Refused => {}
        }
    });
}

/// Waits for `Ctrl+C`, or for `SIGTERM` (what `docker stop` sends).
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut stream) = signal(SignalKind::terminate()) {
            stream.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
}
