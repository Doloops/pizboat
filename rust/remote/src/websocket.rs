use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use tungstenite::{accept, Message};
use std::thread;
use std::time::{Duration};

#[derive(Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: u64,
    
    pub rudder_star: u16,
    pub rudder_port: u16,
    pub motor: u16,
    pub boom: u16,
    pub genoa: u16
}


pub fn websocket_thread(data_mutex: Arc<Mutex<Option<CommandMessage>>>) {
    let server = TcpListener::bind("0.0.0.0:10013").expect("Failed to bind WebSocket server");
    println!("WebSocket server listening on port 10013");

    for stream in server.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Connection error: {}", e);
                continue;
            }
        };

        let data_mutex = Arc::clone(&data_mutex);
        thread::spawn(move || {
            let mut websocket = match accept(stream) {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake error: {}", e);
                    return;
                }
            };

            println!("New WebSocket client connected");

            loop {
                let data = {
                    let locked_data = data_mutex.lock().unwrap();
                    locked_data.clone()
                };

                if let Some(d) = data {
                    match serde_json::to_string(&d) {
                        Ok(json) => {
                            if websocket.send(Message::Text(json)).is_err() {
                                println!("WebSocket client disconnected");
                                break;
                            }
                        }
                        Err(e) => eprintln!("JSON serialization error: {}", e),
                    }
                }

                thread::sleep(Duration::from_millis(40));
            }
        });
    }
}
