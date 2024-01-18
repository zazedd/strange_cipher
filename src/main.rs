use base64::prelude::*;

fn lorenz_attractor(
    x: f64,
    y: f64,
    z: f64,
    sigma: f64,
    rho: f64,
    beta: f64,
    h: f64,
) -> (f64, f64, f64) {
    let new_x = x + h * (-sigma * x - sigma * y);
    let new_y = y + h * (-x * z - rho * x - y);
    let new_z = z + h * (x * y - beta * z);

    (new_x, new_y, new_z)
}

fn generate_key_stream(
    seed: (f64, f64, f64),
    sigma: f64,
    rho: f64,
    beta: f64,
    h: f64,
    n: usize,
) -> Vec<u8> {
    let mut key_stream = Vec::new();
    let mut current_state = seed;

    for _ in 0..n {
        current_state = lorenz_attractor(
            current_state.0,
            current_state.1,
            current_state.2,
            sigma,
            rho,
            beta,
            h,
        );
        let bytes = current_state.1.to_ne_bytes();
        bytes.iter().for_each(|e| key_stream.push(*e));
    }

    key_stream
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
    let seed = (-10.0, -7.0, 35.0);
    let sigma = 10.0;
    let rho = 28.0;
    let beta = 8.0 / 3.0;
    let h = 0.01;
    let n = 8;

    let key_stream = generate_key_stream(seed, sigma, rho, beta, h, n);

    let message = "testing if this cipher is actually working or not, we are still using the same preconditions always";
    let ciphertext = BASE64_STANDARD.encode(encrypt(message, &key_stream));
    let decoded_message = dencrypt(&ciphertext, &key_stream);

    println!("Original Message: {}", message);
    println!("Encrypted Message (Base64): {}", ciphertext);
    println!(
        "Decrypted Message: {}",
        String::from_utf8(decoded_message).unwrap()
    );
}
