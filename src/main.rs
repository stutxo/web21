use futures::{SinkExt, StreamExt};
use gloo_net::websocket::{futures::WebSocket, Message, WebSocketError};
use log::debug;
use nostr::prelude::decrypt;
use nostr::util::nips::nip19::ToBech32;
use nostr::{ClientMessage, Keys, Kind, RelayMessage, SubscriptionFilter};
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    log::debug!("App is starting");

    let my_keys = Keys::generate_from_os_random();

    let bech32_pubkey: String = my_keys.public_key().to_bech32().unwrap();
    debug!("Bech32 PubKey: {}", bech32_pubkey);

    let ws = WebSocket::open("wss://nostr.zebedee.cloud").unwrap();
    let (mut write, mut read) = ws.split();

    let (nostr_tx, mut nostr_rx) = futures::channel::mpsc::channel::<String>(1000);
    let nostr_tx_clone = nostr_tx.clone();

    let id = Uuid::new_v4().to_string();

    let subscribe = ClientMessage::new_req(
        id,
        vec![SubscriptionFilter::new()
            .pubkeys(vec![my_keys.public_key()])
            .kind(Kind::EncryptedDirectMessage)],
    );

    spawn_local(async move {
        nostr_tx_clone
            .clone()
            .send(subscribe.as_json())
            .await
            .unwrap();
    });

    spawn_local(async move {
        while let Some(message) = nostr_rx.next().await {
            debug!("{:?}", message);
            write.send(Message::Text(message)).await.unwrap();
        }
    });

    spawn_local(async move {
        while let Some(result) = read.next().await {
            match result {
                Ok(Message::Text(msg)) => {
                    if let Ok(handled_message) = RelayMessage::from_json(msg) {
                        match handled_message {
                            RelayMessage::Empty => {
                                debug!("Empty message")
                            }
                            RelayMessage::Notice { message } => {
                                debug!("Got a notice: {}", message);
                            }
                            RelayMessage::EndOfStoredEvents { subscription_id: _ } => {
                                debug!("Relay signalled End of Stored Events");
                            }
                            RelayMessage::Ok {
                                event_id,
                                status,
                                message,
                            } => {
                                debug!("Got OK message: {} - {} - {}", event_id, status, message);
                            }
                            RelayMessage::Event {
                                event,
                                subscription_id: _,
                            } => {
                                let msg = decrypt(
                                    &my_keys.secret_key().unwrap(),
                                    &event.pubkey,
                                    event.content,
                                );
                                debug!("{:?}", msg);
                            }
                        }
                    }
                }
                Ok(Message::Bytes(_)) => {}

                Err(e) => match e {
                    WebSocketError::ConnectionError => {}
                    WebSocketError::ConnectionClose(_) => {}
                    WebSocketError::MessageSendError(_) => {}
                    _ => {}
                },
            }
        }
    });
}
