//! API integration tests: drive the axum router in-process.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

// The binary crate exposes its internals for tests via `#[path]` includes
// being unavailable; instead we spin the router through the public main
// entry pieces. Since netpilot-server is a bin crate, we re-build the
// router by launching AppState against a temp dir through the same
// functions the binary uses — exported via a small test shim below.
//
// Simplest robust approach: exercise the HTTP surface over a real
// listener using the compiled binary is heavyweight; instead we link the
// crate's modules directly by including the source files.

#[path = "../src/agent.rs"]
mod agent;
#[path = "../src/api/mod.rs"]
mod api;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/state.rs"]
mod state;

async fn test_app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state = state::AppState::new(dir.path().to_path_buf(), 48000).unwrap();
    (api::router(state), dir)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let resp = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, value)
}

#[tokio::test]
async fn lab_node_link_lifecycle() {
    let (app, _dir) = test_app().await;

    // system status
    let (st, sys) = call(&app, "GET", "/api/system", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(sys["running_nodes"], 0);

    // create lab
    let (st, lab) = call(&app, "POST", "/api/labs", Some(json!({"name": "t1"}))).await;
    assert_eq!(st, StatusCode::OK);
    let lab_id = lab["id"].as_str().unwrap().to_string();

    // empty name rejected
    let (st, _) = call(&app, "POST", "/api/labs", Some(json!({"name": "  "}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // create nodes from a built-in template
    let (st, n1) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R1"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{n1}");
    let (_, n2) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R2"})),
    )
    .await;
    let (id1, id2) = (n1["id"].as_str().unwrap(), n2["id"].as_str().unwrap());

    // duplicate name conflicts
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R1"})),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // unknown template 404s
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "nope"})),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // link them
    let (st, link) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/links"),
        Some(json!({
            "a": {"kind": "node", "node": id1, "iface": 0},
            "b": {"kind": "node", "node": id2, "iface": 0}
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{link}");
    let link_id = link["id"].as_str().unwrap();

    // same interface again -> conflict
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/links"),
        Some(json!({
            "a": {"kind": "node", "node": id1, "iface": 0},
            "b": {"kind": "node", "node": id2, "iface": 1}
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // suspend the link, then verify persisted
    let (st, updated) = call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/links/{link_id}"),
        Some(json!({"suspended": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(updated["suspended"], true);

    let (_, lab_doc) = call(&app, "GET", &format!("/api/labs/{lab_id}"), None).await;
    assert_eq!(lab_doc["links"][link_id]["suspended"], true);

    // startup config set/get
    let (st, _) = call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/nodes/{id1}/config"),
        Some(json!({"config": "set system host-name R1"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, cfg) = call(
        &app,
        "GET",
        &format!("/api/labs/{lab_id}/nodes/{id1}/config"),
        None,
    )
    .await;
    assert_eq!(cfg["config"], "set system host-name R1");

    // interfaces reflect the link
    let (_, ifaces) = call(
        &app,
        "GET",
        &format!("/api/labs/{lab_id}/nodes/{id1}/interfaces"),
        None,
    )
    .await;
    assert_eq!(ifaces[0]["connected"], true);
    assert_eq!(ifaces[0]["name"], "eth0");

    // start fails cleanly without an image
    let (st, err) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes/{id1}/start"),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("no image"));

    // delete node removes its links
    let (st, _) = call(
        &app,
        "DELETE",
        &format!("/api/labs/{lab_id}/nodes/{id1}"),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, links) = call(&app, "GET", &format!("/api/labs/{lab_id}/links"), None).await;
    assert_eq!(links.as_array().unwrap().len(), 0);

    // delete lab
    let (st, _) = call(&app, "DELETE", &format!("/api/labs/{lab_id}"), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = call(&app, "GET", &format!("/api/labs/{lab_id}"), None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lab_locking() {
    let (app, _dir) = test_app().await;
    let (_, lab) = call(&app, "POST", "/api/labs", Some(json!({"name": "locked"}))).await;
    let lab_id = lab["id"].as_str().unwrap().to_string();

    // lock it
    let (st, locked) = call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/lock"),
        Some(json!({"locked": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(locked["locked"], true);

    // edits are rejected
    let (st, err) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R1"})),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert!(err["error"].as_str().unwrap().contains("locked"));

    // unlock and edit again
    call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/lock"),
        Some(json!({"locked": false})),
    )
    .await;
    let (st, _) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R1"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn config_sets_lifecycle() {
    let (app, _dir) = test_app().await;
    let (_, lab) = call(&app, "POST", "/api/labs", Some(json!({"name": "sets"}))).await;
    let lab_id = lab["id"].as_str().unwrap().to_string();
    let (_, node) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "vyos", "name": "R1", "startup_config": "hostname baseline"})),
    )
    .await;
    let node_id = node["id"].as_str().unwrap();

    // snapshot current configs into a set named "golden"
    let (st, view) = call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/config-sets/golden"),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(view["sets"], json!(["golden"]));

    // edit the golden copy independently of the default
    call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/nodes/{node_id}/config?set=golden"),
        Some(json!({"config": "hostname golden"})),
    )
    .await;
    let (_, def) = call(
        &app,
        "GET",
        &format!("/api/labs/{lab_id}/nodes/{node_id}/config"),
        None,
    )
    .await;
    assert_eq!(def["config"], "hostname baseline");
    let (_, gold) = call(
        &app,
        "GET",
        &format!("/api/labs/{lab_id}/nodes/{node_id}/config?set=golden"),
        None,
    )
    .await;
    assert_eq!(gold["config"], "hostname golden");

    // activate the set; deleting it clears activation
    let (_, view) = call(
        &app,
        "PUT",
        &format!("/api/labs/{lab_id}/config-sets"),
        Some(json!({"name": "golden"})),
    )
    .await;
    assert_eq!(view["active"], "golden");
    let (_, view) = call(
        &app,
        "DELETE",
        &format!("/api/labs/{lab_id}/config-sets/golden"),
        None,
    )
    .await;
    assert_eq!(view["active"], "");
    assert_eq!(view["sets"], json!([]));
}

#[tokio::test]
async fn import_export_roundtrip() {
    let (app, _dir) = test_app().await;

    // build a lab
    let (_, lab) = call(&app, "POST", "/api/labs", Some(json!({"name": "round"}))).await;
    let lab_id = lab["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/labs/{lab_id}/nodes"),
        Some(json!({"template": "linux", "name": "h1", "startup_config": "#cloud-config\n"})),
    )
    .await;

    // export zip
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/labs/{lab_id}/export"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let zip_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(zip_bytes.starts_with(b"PK"));

    // import it back — fresh lab id, same content
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import")
                .body(Body::from(zip_bytes.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let imported: Value = serde_json::from_slice(&body).unwrap();
    assert_ne!(imported["id"], lab["id"]);
    assert_eq!(imported["name"], "round");
    assert_eq!(imported["nodes"].as_object().unwrap().len(), 1);

    // import an EVE-NG .unl
    let unl = r#"<?xml version="1.0"?>
<lab name="from-eve" version="1">
  <topology>
    <nodes>
      <node id="1" name="R1" type="qemu" template="vios" image="15.6" ethernet="4" left="100" top="90">
        <interface id="0" name="Gi0/0" type="ethernet" network_id="1"/>
      </node>
      <node id="2" name="R2" type="qemu" template="vyos" ethernet="4" left="300" top="90">
        <interface id="0" name="eth0" type="ethernet" network_id="1"/>
      </node>
    </nodes>
    <networks>
      <network id="1" type="bridge" name="" left="200" top="90" visibility="0"/>
    </networks>
  </topology>
</lab>"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import")
                .body(Body::from(unl))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let eve: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(eve["name"], "from-eve");
    assert_eq!(eve["nodes"].as_object().unwrap().len(), 2);
    assert_eq!(eve["links"].as_object().unwrap().len(), 1);
}
