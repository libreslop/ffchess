use common::*;
use gloo_net::websocket::{futures::WebSocket, Message};
use futures_util::{StreamExt, SinkExt};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NetworkService {
    pub ws: Arc<Mutex<WebSocket>>,
}

impl NetworkService {
    pub async fn new(url: &str, on_message: Callback<ServerMessage>) -> Self {
        let ws = WebSocket::open(url).unwrap();
        let (mut write, mut read) = ws.split();
        let ws_arc = Arc::new(Mutex::new(WebSocket::open(url).unwrap())); // Re-open or use split
        
        // This is a bit tricky with gloo-net. Let's use a simpler approach.
        let ws = WebSocket::open(url).unwrap();
        let (mut write, mut read) = ws.split();
        
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                if let Ok(Message::Text(text)) = msg {
                    if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                        on_message.emit(server_msg);
                    }
                }
            }
        });

        // We need a way to send messages back. 
        // For simplicity, let's just use a channel or a RefCell.
        unimplemented!("Need better architecture for dual-way WS")
    }
}
