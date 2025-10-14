use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use tungstenite::{accept, Message};
use std::thread;
use std::time::{Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryMessage {
    #[serde(rename = "type")]
    msg_type: String,
    timestamp: u64,
    pub wireless_quality: Option<i16>,
    pub latency: Option<u64>,
    pub weight: Option<f32>,
}

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


pub fn websocket_thread(data_mutex: Arc<Mutex<Option<CommandMessage>>>, query_mutex: Arc<Mutex<Option<QueryMessage>>>) {
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
        let query_mutex = Arc::clone(&query_mutex);
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
                let mut timestamp: u64 = 0;
                
                match websocket.read() {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<QueryMessage>(&text) {
                            Ok(query) => {
                                timestamp = query.timestamp;
                                // println!("W {}", query.wireless_quality);
                                {
                                    let mut locked_query = query_mutex.lock().unwrap();
                                    *locked_query = Some(query);
                                }
                            }
                            Err(e) => eprintln!("JSON parse error: {}", e),
                        }
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {
                        eprintln!("Not supported !");
                        break;
                    }
                }

                let data = {
                    let locked_data = data_mutex.lock().unwrap();
                    locked_data.clone()
                };

                if let Some(mut d) = data {
                    d.timestamp = timestamp;
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
