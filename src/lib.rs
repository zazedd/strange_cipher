pub mod common {

    use tungstenite::{util::NonBlockingError, Message, WebSocket};
    pub fn lorenz_attractor(
        x: f64,
        x_prime: Option<f64>,
        y: f64,
        z: f64,
        sigma: f64,
        rho: f64,
        beta: f64,
        h: f64,
    ) -> (f64, f64, f64) {
        let used_x = x_prime.unwrap_or(x);
        let new_x = used_x + (sigma * (y - used_x)) * h;
        let new_y = y + (used_x * (rho - z) - y) * h;
        let new_z = z + (used_x * y - beta * z) * h;

        (new_x, new_y, new_z)
    }

    pub fn send_request<S>(socket: &mut WebSocket<S>, name: &str, request_id: u8) -> ()
    where
        S: std::io::Read + std::io::Write,
    {
        socket
            .send(Message::Binary(vec![request_id]))
            .expect(format!("Unable to send request: {}", name).as_str());

        println!("Sent: {}", name);
    }

    pub fn receive_msg<S>(socket: &mut WebSocket<S>) -> ()
    where
        S: std::io::Read + std::io::Write,
    {
        let msg = socket.read().expect("Error reading message");
        println!("Recieved: {}", msg);
    }

    pub fn read_non_blocking<S>(socket: &mut WebSocket<S>) -> Option<Message>
    where
        S: std::io::Read + std::io::Write,
    {
        match socket.read() {
            Ok(msg) => Some(msg),
            Err(err) => match err.into_non_blocking() {
                Some(e) => panic!("Panic at message {}", e),
                None => None,
            },
        }
    }

    pub fn lin_interp(input: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        y1 + ((y2 - y1) / (x2 - x1)) * (input - x1)
    }

    pub fn interpolate_sigma(rho: f64) -> f64 {
        let rho_range = [24.0, 57.0];
        let sigma_range_for_24 = [6.0, 14.5];
        let sigma_range_for_60 = [4.0, 27.0];

        match rho {
            r if r == rho_range[0] => sigma_range_for_24[0],
            r if r == rho_range[1] => sigma_range_for_60[1],
            _ => {
                let sigma_for_24 = lin_interp(
                    rho,
                    rho_range[0],
                    sigma_range_for_24[0],
                    rho_range[1],
                    sigma_range_for_24[1],
                );
                let sigma_for_60 = lin_interp(
                    rho,
                    rho_range[0],
                    sigma_range_for_60[0],
                    rho_range[1],
                    sigma_range_for_60[1],
                );

                lin_interp(rho, rho_range[0], sigma_for_24, rho_range[1], sigma_for_60)
            }
        }
    }
}

pub mod testing_common {
    use crate::common;

    const SIGMA: f64 = 25.0;
    const RHO: f64 = 2.0;
    const BETA: f64 = 8.0 / 3.0;
    const H: f64 = 0.01;

    pub fn generate_key_stream() -> Vec<u8> {
        let mut key_stream = Vec::new();
        while key_stream.len() < 16 {
            let state = common::lorenz_attractor(-10.0, None, -7.0, 35.0, SIGMA, RHO, BETA, H);

            let bytes = state.1.to_ne_bytes();
            bytes.iter().for_each(|e| key_stream.push(*e));
        }
        key_stream
    }
}
