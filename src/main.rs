mod character_info;
mod config;
mod db;
mod events;
mod names;
mod poller;

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderValue};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use config::Config;
use events::RosterMessage;
use names::NameTables;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    /// The most recently broadcast roster, as pre-serialized JSON, so a
    /// newly connected client can render the agent field immediately
    /// instead of waiting up to one poll interval for the next update.
    latest_roster: Arc<RwLock<Option<String>>>,
}

#[tokio::main]
async fn main() {
    let config = Config::load();
    let names = NameTables::load();

    println!(
        "Connecting to {}:{}/{}...",
        config.database.host, config.database.port, config.database.database
    );
    let pool = db::connect(&config.database).await;
    println!("Connected.");

    let (tx, _rx) = broadcast::channel::<String>(256);
    let latest_roster = Arc::new(RwLock::new(None));
    let state = Arc::new(AppState {
        tx: tx.clone(),
        latest_roster: latest_roster.clone(),
    });

    let poll_interval = config.server.poll_interval_secs;
    let event_tx = tx.clone();
    let roster_tx = tx.clone();
    tokio::spawn(async move {
        poller::run(
            pool,
            names,
            poll_interval,
            move |events| {
                for event in events {
                    if let Ok(json) = serde_json::to_string(event) {
                        // Ignore send errors -- they just mean no client is
                        // connected right now, which is fine.
                        let _ = event_tx.send(json);
                    }
                }
            },
            move |roster| {
                let msg = RosterMessage::new(roster.to_vec());
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = roster_tx.send(json.clone());
                    let latest_roster = latest_roster.clone();
                    tokio::spawn(async move {
                        *latest_roster.write().await = Some(json);
                    });
                }
            },
        )
        .await;
    });

    // The web/ assets get iterated on live (this is a presentation console
    // under active development) -- without this, browsers happily cache
    // app.js/index.html indefinitely and a plain refresh keeps serving
    // stale JS even after the file on disk changed (confirmed: a full
    // navigate reused a cached app.js with no visible sign anything was wrong).
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("web"))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.server.http_port);
    println!("Listening on http://{addr} (poll interval {poll_interval}s)");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("failed to bind HTTP port");
    axum::serve(listener, app).await.expect("server error");
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    if let Some(roster_json) = state.latest_roster.read().await.clone() {
        if socket.send(Message::Text(roster_json)).await.is_err() {
            return;
        }
    }

    let mut rx = state.tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}
