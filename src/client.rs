use std::io::{self, Write};

use tungstenite::{connect, Message};
use url::Url;

use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

use base64::prelude::*;
use strange_cipher::common;

enum ClientState {
    Unverified,
    Waiting {
        rho: f64,
        sigma: f64,
    },
    Syncing {
        rho: f64,
        sigma: f64,
    },
    Encrypting {
        rho: f64,
        sigma: f64,
    },
    Encrypted {
        rho: f64,
        sigma: f64,
        ciphertext: String,
    },
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

pub fn main() {
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
    let beta = 8.0 / 3.0;
    let h = 0.01;
    let mut stream_state = ClientState::Unverified;
    let mut key_stream = Vec::new();

    let mut state = (0.0, 0.0, 0.0);
    let mut input = String::new();

    loop {
        match stream_state {
            ClientState::Unverified => {
                println!("Starting Key exchange");

                let client_secret_key = EphemeralSecret::random_from_rng(OsRng);
                let client_public_key = PublicKey::from(&client_secret_key);
                let shared_secret: SharedSecret;

                socket
                    .send(Message::Binary(client_public_key.to_bytes().to_vec()))
                    .expect("Could not send public key");

                match socket.read() {
                    Ok(Message::Binary(server_public_key_bytes)) => {
                        let mut byte_array: [u8; 32] = [0; 32];
                        byte_array[..32].copy_from_slice(server_public_key_bytes.as_slice());
                        let server_public_key = PublicKey::from(byte_array);

                        shared_secret = client_secret_key.diffie_hellman(&server_public_key);
                    }
                    _ => panic!("Recieved Invalid Key"),
                }

                let result = shared_secret.to_bytes()[10];
                let rho = common::lin_interp(result as f64, 0.0, 24.0, 255.0, 57.0);
                let sigma = common::interpolate_sigma(rho);

                println!("rho = {}", rho);
                println!("sigma = {}", sigma);

                state = common::lorenz_attractor(seed.0, None, seed.1, seed.2, sigma, rho, beta, h);

                stream_state = ClientState::Waiting { rho, sigma };
            }
            ClientState::Waiting { rho, sigma } => {
                match socket.get_mut() {
                    tungstenite::stream::MaybeTlsStream::Plain(stream) => {
                        stream.set_nonblocking(false)
                    }
                    _ => unimplemented!(),
                }
                .expect("Could not make socket non-blocking");

                print!("Type a message you want to encrypt (Empty to Cancel): ");
                input.clear();
                io::stdout().flush().unwrap();
                io::stdin().read_line(&mut input).unwrap();
                let input = input.trim().to_string();

                if input == "" {
                    common::send_request(&mut socket, "Cancel Request", 0);
                    break;
                }

                common::send_request(&mut socket, "Sync Request", 1);
                common::receive_msg(&mut socket);
                stream_state = ClientState::Syncing { rho, sigma };
            }
            ClientState::Syncing { rho, sigma } => {
                match socket.get_mut() {
                    tungstenite::stream::MaybeTlsStream::Plain(stream) => {
                        stream.set_nonblocking(true)
                    }
                    _ => unimplemented!(),
                }
                .expect("Could not make socket non-blocking");

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

                match common::read_non_blocking(&mut socket) {
                    Some(Message::Binary(v)) if v.as_slice() == [2] => {
                        println!("Server finished syncing. Encrypting now");
                        stream_state = ClientState::Encrypting { rho, sigma };
                    }
                    _ => (),
                }
            }

            ClientState::Encrypting { rho, sigma } => {
                if key_stream.len() == 16 {
                    let ciphertext = BASE64_STANDARD.encode(encrypt(input.as_str(), &key_stream));
                    stream_state = ClientState::Encrypted {
                        rho,
                        sigma,
                        ciphertext,
                    };
                    continue;
                }

                state =
                    common::lorenz_attractor(state.0, None, state.1, state.2, sigma, rho, beta, h);

                let bytes = state.1.to_ne_bytes();
                bytes.iter().for_each(|e| key_stream.push(*e));
            }

            ClientState::Encrypted {
                rho,
                sigma,
                ref ciphertext,
            } => {
                println!("Finished encrypting with message = {}", ciphertext);
                println!("Sending encrypted message");

                common::send_request(&mut socket, "Encryption Completed", 3);

                socket
                    .send(Message::Text(ciphertext.to_string()))
                    .expect("Could not send ciphertext");

                let fst: &[u8] = &key_stream[0..8];
                fst.iter().for_each(|&item| {
                    socket
                        .send(Message::Binary(vec![item]))
                        .expect("Could not send byte")
                });

                stream_state = ClientState::Waiting { rho, sigma };
            }
        }
    }

    println!("Done. Bye bye");
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use strange_cipher::testing_common::generate_key_stream;

    #[test]
    fn test_encrypt() {
        let key_stream = generate_key_stream();
        let message = "Hello, Testing!";

        let encrypted = BASE64_STANDARD.encode(encrypt(message, &key_stream));

        assert_eq!("QrLPHFImLZRvpNcZU20s", encrypted);
    }

    #[test]
    fn test_encrypt_empty_message() {
        let key_stream = generate_key_stream();
        let message = "";

        let encrypted = BASE64_STANDARD.encode(encrypt(message, &key_stream));

        assert_eq!("", encrypted);
    }
}
