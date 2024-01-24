use std::io::{self, Write};
use std::net::TcpStream;

use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

use base64::prelude::*;
use common::common;

enum ClientState {
    Syncing,
    Encrypting,
    Encrypted(String),
}

fn receive_msg(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> () {
    let msg = socket.read().expect("Error reading message");
    println!("Recieved: {}", msg);
}

fn encrypt(message: &str, key_stream: &[u8]) -> Vec<u8> {
    let message_bytes = message.as_bytes();
    let mut ciphertext = Vec::new();

    for (i, &byte) in message_bytes.iter().enumerate() {
        let key_byte = key_stream[i % key_stream.len()];
        let encrypted_byte = byte ^ key_byte;
        ciphertext.push(encrypted_byte);
    }

    ciphertext
}

fn main() {
    env_logger::init();

    let (mut socket, response) =
        connect(Url::parse("ws://localhost:3012/socket").unwrap()).expect("Can't connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (ref header, _value) in response.headers() {
        println!("* {}", header);
    }

    let seed = (-10.0, -7.0, 35.0);
    let sigma = 10.0;
    let rho = 28.0;
    let beta = 8.0 / 3.0;
    let h = 0.01;
    let mut stream_state = ClientState::Syncing;
    let mut key_stream = Vec::new();

    let mut state = common::lorenz_attractor(seed.0, None, seed.1, seed.2, sigma, rho, beta, h);

    let mut input = String::new();
    print!("Type a message you want to encrypt: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_string();

    common::send_request(&mut socket, "Sync Request", 1);
    receive_msg(&mut socket);

    match socket.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(stream) => stream.set_nonblocking(true),
        _ => unimplemented!(),
    }
    .expect("Could not make socket non-blocking");

    loop {
        match stream_state {
            ClientState::Syncing => {
                state =
                    common::lorenz_attractor(state.0, None, state.1, state.2, sigma, rho, beta, h);
                socket
                    .send(Message::Binary(state.0.to_ne_bytes().to_vec()))
                    .expect("Could not send x coordinate");
                socket
                    .send(Message::Binary(state.1.to_ne_bytes().to_vec()))
                    .expect("Could not send y coordinate");
                socket
                    .send(Message::Binary(state.2.to_ne_bytes().to_vec()))
                    .expect("Could not send z coordinate");

                println!("state = {:?}", state);

                match common::read_non_blocking(&mut socket) {
                    Some(Message::Binary(v)) if v.as_slice() == [2] => {
                        println!("Server finished syncing. Encrypting now");
                        stream_state = ClientState::Encrypting;
                    }
                    _ => (),
                }

                // Allows for a new message to be created, making the sync better, but
                // slower
                // sleep(Duration::new(0, 5000));
            }

            ClientState::Encrypting => {
                if key_stream.len() == 16 {
                    let ciphertext = BASE64_STANDARD.encode(encrypt(input.as_str(), &key_stream));
                    stream_state = ClientState::Encrypted(ciphertext);
                    continue;
                }

                state =
                    common::lorenz_attractor(state.0, None, state.1, state.2, sigma, rho, beta, h);

                let bytes = state.1.to_ne_bytes();
                bytes.iter().for_each(|e| key_stream.push(*e));
            }

            ClientState::Encrypted(ref ciphertext) => {
                println!("Finished encrypting with message = {}", ciphertext);
                println!("Sending encrypted message");

                common::send_request(&mut socket, "Encryption Completed", 3);

                socket
                    .send(Message::Text(ciphertext.to_string()))
                    .expect("Could not send ciphertext");

                let fst: u8 = key_stream.first().expect("No first").to_owned();
                socket
                    .send(Message::Binary(vec![fst]))
                    .expect("Could not send fst");

                break;
            }
        }
    }

    println!("Done. Bye bye");
}
