use std::{net::TcpListener, thread::spawn, time::SystemTime};

use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    Message,
};

use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey};

use base64::prelude::*;
use strange_cipher::common;

enum ServerState {
    Unverified,
    Unsynced {
        rho: f64,
        sigma: f64,
    },
    Syncing {
        rho: f64,
        sigma: f64,
    },
    Synced {
        rho: f64,
        sigma: f64,
    },
    Encrypted {
        rho: f64,
        sigma: f64,
    },
    Decrypted {
        rho: f64,
        sigma: f64,
        plaintext: String,
    },
}

fn decrypt(base64_message: &str, key_stream: &[u8]) -> Vec<u8> {
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

    for (i, stream) in server.incoming().enumerate() {
        spawn(move || {
            let callback = |req: &Request, response: Response| {
                println!("New Client connected");
                println!("The request's path is: {}", req.uri().path());
                println!("The request's headers are:");
                for (ref header, _value) in req.headers() {
                    println!("* {}", header);
                }

                // Let's add an additional header to our response to the client.
                // let headers = response.headers_mut();
                // headers.append("MyCustomHeader", ":)".parse().unwrap());
                // headers.append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());

                Ok(response)
            };
            let mut websocket = accept_hdr(stream.unwrap(), callback).unwrap();

            let mut stream_state = ServerState::Unverified;

            let mut seed = (1.0, 1.0, 2.0);
            let beta = 8.0 / 3.0;
            let h = 0.01;
            let mut last_y = 0.;
            let mut last_z = 0.;
            let mut sync_count = 0;
            let mut key_stream = Vec::new();
            let mut time = SystemTime::now();

            loop {
                match stream_state {
                    ServerState::Unverified => {
                        println!("Starting Key exchange with Client {}", i);

                        let server_secret_key = EphemeralSecret::random_from_rng(OsRng);
                        let server_public_key = PublicKey::from(&server_secret_key);
                        let shared_secret;

                        match websocket.read() {
                            Ok(Message::Binary(client_public_key_bytes)) => {
                                let mut byte_array: [u8; 32] = [0; 32];
                                byte_array[..32]
                                    .copy_from_slice(client_public_key_bytes.as_slice());
                                let client_public_key = PublicKey::from(byte_array);

                                shared_secret =
                                    server_secret_key.diffie_hellman(&client_public_key);
                            }
                            _ => panic!("Recieved Invalid Key"),
                        }

                        websocket
                            .send(Message::Binary(server_public_key.to_bytes().to_vec()))
                            .expect("Could not send public key");

                        let result = shared_secret.to_bytes()[10];
                        let rho = common::lin_interp(result as f64, 0.0, 24.0, 255.0, 57.0);
                        let sigma = common::interpolate_sigma(rho);

                        println!("rho = {}", rho);
                        println!("sigma = {}", sigma);

                        seed = common::lorenz_attractor(
                            seed.0, None, seed.1, seed.2, sigma, rho, beta, h,
                        );

                        stream_state = ServerState::Unsynced { rho, sigma };
                    }
                    ServerState::Unsynced { rho, sigma } => {
                        websocket
                            .get_mut()
                            .set_nonblocking(true)
                            .expect("Couldn't make socket non-blocking");

                        let (new_x, new_y, new_z) = common::lorenz_attractor(
                            seed.0, None, seed.1, seed.2, sigma, rho, beta, h,
                        );
                        seed = (new_x, new_y, new_z);

                        match common::read_non_blocking(&mut websocket) {
                            Some(Message::Binary(v)) => match v.as_slice() {
                                [1] => {
                                    time = SystemTime::now();
                                    println!("Received: Sync Request");
                                    websocket
                                        .send(Message::Text("Sync Request approved".to_string()))
                                        .unwrap();

                                    println!("Sent: Sync Request approved");
                                    stream_state = ServerState::Syncing { rho, sigma };
                                }
                                [0] => {
                                    println!("Received: Cancel Request");
                                    println!("Client Number {} Left", i);
                                    break;
                                }
                                _ => panic!("Invalid Request Received"),
                            },
                            _ => (),
                        }
                    }
                    ServerState::Syncing { rho, sigma } => {
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
                                let (new_x, new_y, new_z) = common::lorenz_attractor(
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
                                println!("{}, {}, {}", new_x, new_y, new_z);
                                println!("{}, {}", rho, sigma);

                                if last_y == y_prime && last_z == z_prime {
                                    println!("Synced!!");
                                    sync_count += 1;
                                    if sync_count == 100 {
                                        println!("Sync Complete");
                                        common::send_request(&mut websocket, "Sync Complete", 2);
                                        stream_state = ServerState::Synced { rho, sigma };
                                    }
                                } else {
                                    sync_count = 0;
                                }

                                last_y = new_y;
                                last_z = new_z;
                            }
                            _ => panic!("Received invalid data format"),
                        }
                    }
                    ServerState::Synced { rho, sigma } => {
                        websocket
                            .get_mut()
                            .set_nonblocking(true)
                            .expect("Couldn't make socket non-blocking");

                        let (new_x, new_y, new_z) = common::lorenz_attractor(
                            seed.0, None, seed.1, seed.2, sigma, rho, beta, h,
                        );
                        seed = (new_x, new_y, new_z);

                        let bytes = seed.1.to_ne_bytes();
                        bytes.iter().for_each(|e| key_stream.push(*e));

                        match common::read_non_blocking(&mut websocket) {
                            Some(Message::Binary(v)) if v.as_slice() == [3] => {
                                stream_state = ServerState::Encrypted { rho, sigma }
                            }
                            _ => (),
                        }
                    }
                    ServerState::Encrypted { rho, sigma } => {
                        websocket
                            .get_mut()
                            .set_nonblocking(false)
                            .expect("Couldn't make socket blocking");
                        match websocket.read() {
                            Ok(Message::Text(ciphertext)) => {
                                println!("Received ciphertext = {}", ciphertext);

                                let mut first_float = Vec::new();

                                for _ in 0..8 {
                                    match websocket.read() {
                                        Ok(Message::Binary(byte)) => first_float.push(byte),
                                        _ => panic!("Could not read byte"),
                                    }
                                }

                                let first_float =
                                    first_float.into_iter().flatten().collect::<Vec<u8>>();

                                match key_stream
                                    .windows(8)
                                    .position(|window| window == &first_float[0..8])
                                {
                                    Some(index) => {
                                        let new_key_stream = key_stream[index..index + 16].to_vec();
                                        let decoded_message = decrypt(&ciphertext, &new_key_stream);
                                        stream_state = ServerState::Decrypted {
                                            rho,
                                            sigma,
                                            plaintext: String::from_utf8(decoded_message).unwrap(),
                                        }
                                    }
                                    None => {
                                        panic!("Item {:?} not found in the vector.", first_float)
                                    }
                                }
                            }
                            _ => panic!("Invalid message received"),
                        }
                    }
                    ServerState::Decrypted {
                        rho,
                        sigma,
                        ref plaintext,
                    } => {
                        println!("Decoded message from client {}: {}", i, plaintext.trim());
                        println!("Took: {}ms", time.elapsed().unwrap().as_millis());

                        // desync the attractors
                        seed = (seed.0 + 0.1, seed.1 - 0.1, seed.2 + 0.1);
                        stream_state = ServerState::Unsynced { rho, sigma };
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use strange_cipher::testing_common::generate_key_stream;

    #[test]
    fn test_decrypt_synced() {
        let key_stream = generate_key_stream();

        let encrypted_message = "QrLPHFImLZRvpNcZU20s";
        let decrypted = String::from_utf8(decrypt(encrypted_message, &key_stream)).unwrap();
        assert_eq!("Hello, Testing!", decrypted);
    }

    #[test]
    fn test_decrypt_empty_message_synced() {
        let key_stream = generate_key_stream();

        let encrypted_message = "";
        let decrypted = String::from_utf8(decrypt(encrypted_message, &key_stream)).unwrap();
        assert_eq!("", decrypted);
    }

    // TODO: sync test
    // TODO: sync + decrypt test
}
