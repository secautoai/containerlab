//! REST + WebSocket API surface.

pub mod admin;
pub mod auth;
pub mod capture;
pub mod interop;
pub mod labs;
pub mod nodes;
pub mod system;
pub mod topology;
pub mod ws;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::AppState;

/// Uploads (base images, lab archives) may be multi-GB.
const UPLOAD_LIMIT: usize = 16 * 1024 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    Router::new()
        // system
        .route("/api/system", get(system::status))
        .route("/api/templates", get(system::templates))
        .route("/api/images", get(system::images))
        .route(
            "/api/images/{template}/{version}/{filename}",
            put(system::upload_image).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)),
        )
        .route("/api/images/{template}/{version}", delete(system::delete_image))
        .route(
            "/api/images/docker/{template}",
            put(system::upload_docker_image).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)),
        )
        .route("/api/labs/{lab}/stats", get(system::lab_stats))
        // labs
        .route("/api/labs", get(labs::list).post(labs::create))
        .route(
            "/api/labs/{lab}",
            get(labs::get_lab).put(labs::update).delete(labs::remove),
        )
        .route("/api/labs/{lab}/clone", post(labs::clone_lab))
        .route("/api/labs/{lab}/lock", put(labs::set_lock))
        .route(
            "/api/labs/{lab}/config-sets",
            get(labs::config_sets).put(labs::activate_config_set),
        )
        .route(
            "/api/labs/{lab}/config-sets/{name}",
            post(labs::snapshot_config_set).delete(labs::delete_config_set),
        )
        .route("/api/labs/{lab}/start", post(labs::start))
        .route("/api/labs/{lab}/stop", post(labs::stop))
        // nodes
        .route(
            "/api/labs/{lab}/nodes",
            get(nodes::list).post(nodes::create),
        )
        .route(
            "/api/labs/{lab}/nodes/{node}",
            get(nodes::get_node)
                .put(nodes::update)
                .delete(nodes::remove),
        )
        .route("/api/labs/{lab}/nodes/{node}/start", post(nodes::start))
        .route("/api/labs/{lab}/nodes/{node}/stop", post(nodes::stop))
        .route("/api/labs/{lab}/nodes/{node}/wipe", post(nodes::wipe))
        .route(
            "/api/labs/{lab}/nodes/{node}/config",
            get(nodes::get_config).put(nodes::set_config),
        )
        .route(
            "/api/labs/{lab}/nodes/{node}/config/export",
            post(nodes::export_config),
        )
        .route("/api/labs/{lab}/nodes/{node}/exec", post(nodes::exec))
        .route(
            "/api/labs/{lab}/nodes/{node}/interfaces",
            get(nodes::interfaces),
        )
        // networks / links / annotations
        .route(
            "/api/labs/{lab}/networks",
            get(topology::list_networks).post(topology::create_network),
        )
        .route(
            "/api/labs/{lab}/networks/{net}",
            put(topology::update_network).delete(topology::remove_network),
        )
        .route(
            "/api/labs/{lab}/links",
            get(topology::list_links).post(topology::create_link),
        )
        .route(
            "/api/labs/{lab}/links/{link}",
            put(topology::update_link).delete(topology::remove_link),
        )
        .route(
            "/api/labs/{lab}/annotations",
            get(topology::list_annotations).post(topology::create_annotation),
        )
        .route(
            "/api/labs/{lab}/annotations/{ann}",
            put(topology::update_annotation).delete(topology::remove_annotation),
        )
        // capture
        .route(
            "/api/labs/{lab}/nodes/{node}/interfaces/{iface}/capture/start",
            post(capture::start),
        )
        .route(
            "/api/labs/{lab}/nodes/{node}/interfaces/{iface}/capture/stop",
            post(capture::stop),
        )
        .route(
            "/api/labs/{lab}/nodes/{node}/interfaces/{iface}/capture.pcap",
            get(capture::download),
        )
        .route(
            "/api/labs/{lab}/nodes/{node}/interfaces/{iface}/capture/summary",
            get(capture::summary),
        )
        // import / export
        .route("/api/labs/{lab}/export", get(interop::export_lab))
        .route(
            "/api/import",
            post(interop::import_lab).layer(DefaultBodyLimit::max(256 * 1024 * 1024)),
        )
        // auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        // users (admin)
        .route("/api/users", get(admin::list_users).post(admin::create_user))
        .route("/api/users/{user}", put(admin::update_user))
        // lab sharing
        .route(
            "/api/labs/{lab}/shares",
            get(admin::get_shares).put(admin::update_share),
        )
        .route("/api/labs/{lab}/shares/{username}", delete(admin::revoke_share))
        // agent sessions (persisted history)
        .route("/api/labs/{lab}/sessions", get(admin::list_sessions))
        .route("/api/labs/{lab}/sessions/{session}", get(admin::get_session))
        // websockets
        .route("/api/ws/events", get(ws::events))
        .route("/api/ws/console/{lab}/{node}", get(ws::console))
        .route("/api/ws/vnc/{lab}/{node}", get(ws::vnc))
        .route("/api/ws/agent/{lab}", get(ws::agent))
        .with_state(state)
}
