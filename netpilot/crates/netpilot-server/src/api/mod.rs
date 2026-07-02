//! REST + WebSocket API surface.

pub mod capture;
pub mod interop;
pub mod labs;
pub mod nodes;
pub mod system;
pub mod topology;
pub mod ws;

use axum::routing::{get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        // system
        .route("/api/system", get(system::status))
        .route("/api/templates", get(system::templates))
        .route("/api/images", get(system::images))
        // labs
        .route("/api/labs", get(labs::list).post(labs::create))
        .route(
            "/api/labs/{lab}",
            get(labs::get_lab).put(labs::update).delete(labs::remove),
        )
        .route("/api/labs/{lab}/clone", post(labs::clone_lab))
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
        // import / export
        .route("/api/labs/{lab}/export", get(interop::export_lab))
        .route("/api/import", post(interop::import_lab))
        // websockets
        .route("/api/ws/events", get(ws::events))
        .route("/api/ws/console/{lab}/{node}", get(ws::console))
        .route("/api/ws/vnc/{lab}/{node}", get(ws::vnc))
        .route("/api/ws/agent/{lab}", get(ws::agent))
        .with_state(state)
}
