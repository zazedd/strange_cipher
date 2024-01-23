use std::{
    net::TcpListener,
    net::TcpStream,
    thread::{sleep, spawn},
    time::{Duration, SystemTime},
};

use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    stream::MaybeTlsStream,
    util::NonBlockingError,
    Message, WebSocket,
};

use base64::prelude::*;

enum ServerState {
    Unsynced,
    Syncing,
    Synced,
    Encrypted,
    Decrypted(String),
}

fn lorenz_attractor(
    x: f64,
    x_prime: Option<f64>,
    y: f64,
    z: f64,
    sigma: f64,
    rho: f64,
    beta: f64,
    h: f64,
) -> (f64, f64, f64) {
    match x_prime {
        None => {
            let new_x = x + (sigma * (y - x)) * h;
            let new_y = y + (x * (rho - z) - y) * h;
            let new_z = z + (x * y - beta * z) * h;

            (new_x, new_y, new_z)
        }
        Some(sync_x) => {
            let new_x = sync_x + (sigma * (y - sync_x)) * h;
            let new_y = y + (sync_x * (rho - z) - y) * h;
            let new_z = z + (sync_x * y - beta * z) * h;

            (new_x, new_y, new_z)
        }
    }
}

fn send_request(socket: &mut WebSocket<TcpStream>, name: &str, request_id: u8) -> () {
    socket
        .send(Message::Binary(vec![request_id]))
        .expect(format!("Unable to send request: {}", name).as_str());

    println!("Sent: {}", name);
}

fn read_non_blocking(socket: &mut WebSocket<TcpStream>) -> Option<Message> {
    match socket.read() {
        Ok(msg) => Some(msg),
        Err(err) => match err.into_non_blocking() {
            Some(e) => panic!("Panic at message {}", e),
            None => None,
        },
    }
}

fn dencrypt(base64_message: &str, key_stream: &[u8]) -> Vec<u8> {
    let encrypted_message_bytes = BASE64_STANDARD.decode(base64_message).unwrap();
    let mut decrypted_message = Vec::new();

    for (i, &byte) in encrypted_message_bytes.iter().enumerate() {
        let key_byte = key_stream[i % key_stream.len()];
        let encrypted_byte = byte ^ key_byte;
        decrypted_message.push(encrypted_byte);
    }

    decrypted_message
}

fn main() {
    env_logger::init();

    let server = TcpListener::bind("127.0.0.1:3012").unwrap();
    println!("Server Started");

    for stream in server.incoming() {
        spawn(move || {
            let callback = |req: &Request, mut response: Response| {
                println!("Received a new ws handshake");
                println!("The request's path is: {}", req.uri().path());
                println!("The request's headers are:");
                for (ref header, _value) in req.headers() {
                    println!("* {}", header);
                }

                // Let's add an additional header to our response to the client.
                let headers = response.headers_mut();
                headers.append("MyCustomHeader", ":)".parse().unwrap());
                headers.append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());

                Ok(response)
            };
            let mut websocket = accept_hdr(stream.unwrap(), callback).unwrap();

            let mut stream_state = ServerState::Unsynced;

            websocket
                .get_mut()
                .set_nonblocking(true)
                .expect("Couldn't make socket non-blocking");

            let mut seed = (0.0, 1.0, 2.0);
            let sigma = 10.0;
            let rho = 28.0;
            let beta = 8.0 / 3.0;
            let h = 0.01;
            let mut last_y = 0.;
            let mut last_z = 0.;
            let mut sync_count = 0;
            let mut key_stream = Vec::new();
            let mut time = SystemTime::now();

            loop {
                match stream_state {
                    ServerState::Unsynced => {
                        let (new_x, new_y, new_z) =
                            lorenz_attractor(seed.0, None, seed.1, seed.2, sigma, rho, beta, h);
                        seed = (new_x, new_y, new_z);

                        // println!("seed = {:?}", seed);

                        if let Some(Message::Binary(v)) = read_non_blocking(&mut websocket) {
                            if v.as_slice() == [1] {
                                time = SystemTime::now();
                                println!("Received: Sync Request");
                                websocket
                                    .send(Message::Text("Sync Request approved".to_string()))
                                    .unwrap();
                                stream_state = ServerState::Syncing;
                            }
                        }

                        // sleep(Duration::new(0, 5000));
                    }
                    ServerState::Syncing => {
                        websocket
                            .get_mut()
                            .set_nonblocking(false)
                            .expect("Couldn't make socket blocking");

                        match (websocket.read(), websocket.read(), websocket.read()) {
                            (
                                Ok(Message::Binary(x_prime_msg)),
                                Ok(Message::Binary(y_prime_msg)),
                                Ok(Message::Binary(z_prime_msg)),
                            ) => {
                                let x_prime =
                                    f64::from_ne_bytes(x_prime_msg[0..8].try_into().unwrap());
                                let y_prime =
                                    f64::from_ne_bytes(y_prime_msg[0..8].try_into().unwrap());
                                let z_prime =
                                    f64::from_ne_bytes(z_prime_msg[0..8].try_into().unwrap());
                                let (new_x, new_y, new_z) = lorenz_attractor(
                                    seed.0,
                                    Some(x_prime),
                                    seed.1,
                                    seed.2,
                                    sigma,
                                    rho,
                                    beta,
                                    h,
                                );
                                seed = (new_x, new_y, new_z);

                                // println!("seed = {:?}", seed);

                                if last_y == y_prime && last_z == z_prime {
                                    sync_count += 1;
                                    println!("Lorenz Attractors are synced");
                                    if sync_count == 100 {
                                        send_request(&mut websocket, "Sync Complete", 2);
                                        stream_state = ServerState::Synced;
                                    }
                                } else {
                                    sync_count = 0;
                                }
                                // Allows for a new message to be created, making the sync better, but
                                // slower
                                // sleep(Duration::new(0, 5000));
                                last_y = new_y;
                                last_z = new_z;
                            }
                            _ => panic!("Received invalid data format"),
                        }
                    }
                    ServerState::Synced => {
                        websocket
                            .get_mut()
                            .set_nonblocking(true)
                            .expect("Couldn't make socket non-blocking");

                        let (new_x, new_y, new_z) =
                            lorenz_attractor(seed.0, None, seed.1, seed.2, sigma, rho, beta, h);
                        seed = (new_x, new_y, new_z);

                        let bytes = seed.1.to_ne_bytes();
                        bytes.iter().for_each(|e| key_stream.push(*e));

                        match read_non_blocking(&mut websocket) {
                            Some(Message::Binary(v)) if v.as_slice() == [3] => {
                                stream_state = ServerState::Encrypted
                            }
                            _ => (),
                        }
                    }
                    ServerState::Encrypted => {
                        websocket
                            .get_mut()
                            .set_nonblocking(false)
                            .expect("Couldn't make socket blocking");
                        match (websocket.read(), websocket.read()) {
                            (Ok(Message::Text(ciphertext)), Ok(Message::Binary(fst))) => {
                                println!("Received ciphertext = {}", ciphertext);

                                let first_item = fst.first().unwrap().to_owned();
                                match key_stream.iter().position(|&item| item == first_item) {
                                    Some(index) => {
                                        let new_key_stream = key_stream[index..index + 16].to_vec();
                                        let decoded_message =
                                            dencrypt(&ciphertext, &new_key_stream);
                                        stream_state = ServerState::Decrypted(
                                            String::from_utf8(decoded_message).unwrap(),
                                        );
                                    }
                                    None => panic!("Item {} not found in the vector.", first_item),
                                }
                            }
                            _ => panic!("Invalid message received"),
                        }
                    }
                    ServerState::Decrypted(ref plaintext) => {
                        println!("Decoded message: {}", plaintext);
                        println!("Took: {}ms", time.elapsed().unwrap().as_millis());
                        break;
                    }
                }
            }
        });
    }
}
