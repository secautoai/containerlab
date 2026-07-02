//! netpilot — modern network emulator server.
//!
//! Usage:
//!   netpilot [--data <dir>] [--listen <addr:port>] [--ui <dist-dir>]
//!
//! Environment:
//!   ANTHROPIC_API_KEY   enables the AI agent mode
//!   NETPILOT_AI_MODEL   overrides the agent model
//!   RUST_LOG            log filtering (default info)

mod agent;
mod api;
mod error;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Debug)]
struct Options {
    data_dir: PathBuf,
    listen: SocketAddr,
    ui_dir: Option<PathBuf>,
    port_base: u16,
    datapath: state::DatapathMode,
}

fn parse_args() -> Options {
    let mut opts = Options {
        data_dir: std::env::var_os("NETPILOT_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./netpilot-data")),
        listen: "127.0.0.1:8090".parse().unwrap(),
        ui_dir: None,
        port_base: 45000,
        datapath: state::DatapathMode::UdpSwitch,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => {
                if let Some(v) = args.next() {
                    opts.data_dir = v.into();
                }
            }
            "--listen" => {
                if let Some(v) = args.next() {
                    opts.listen = v.parse().expect("--listen must be addr:port");
                }
            }
            "--ui" => {
                if let Some(v) = args.next() {
                    opts.ui_dir = Some(v.into());
                }
            }
            "--port-base" => {
                if let Some(v) = args.next() {
                    opts.port_base = v.parse().expect("--port-base must be a port");
                }
            }
            "--datapath" => match args.next().as_deref() {
                Some("udp") => opts.datapath = state::DatapathMode::UdpSwitch,
                Some("bridge") => opts.datapath = state::DatapathMode::Bridge,
                other => {
                    eprintln!("--datapath must be 'udp' or 'bridge' (got {other:?})");
                    std::process::exit(2);
                }
            },
            "--help" | "-h" => {
                println!(
                    "netpilot {}\n\nUSAGE:\n  netpilot [--data DIR] [--listen ADDR:PORT] [--ui DIST_DIR] [--port-base PORT] [--datapath udp|bridge]",
                    env!("CARGO_PKG_VERSION")
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other} (try --help)");
                std::process::exit(2);
            }
        }
    }
    opts
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,netpilot=debug".into()),
        )
        .init();

    let opts = parse_args();
    tracing::info!("data dir: {}", opts.data_dir.display());

    let state =
        state::AppState::with_datapath(opts.data_dir.clone(), opts.port_base, opts.datapath)?;
    tracing::info!("datapath: {:?}", opts.datapath);
    tracing::info!(
        "kvm: {} · labs: {}",
        state.kvm(),
        state.store.list().map(|l| l.len()).unwrap_or(0)
    );

    let mut app: Router = api::router(state);

    // Serve the built UI when present (single-binary deployment).
    let ui_dir = opts.ui_dir.or_else(|| {
        let candidate = PathBuf::from("ui/dist");
        candidate.join("index.html").exists().then_some(candidate)
    });
    if let Some(dir) = ui_dir {
        tracing::info!("serving UI from {}", dir.display());
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(index)));
    }

    app = app.layer(CorsLayer::permissive());

    tracing::info!("listening on http://{}", opts.listen);
    let listener = tokio::net::TcpListener::bind(opts.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
