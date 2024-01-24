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
        let used_x = match x_prime {
            Some(sync_x) => sync_x,
            None => x,
        };
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
}

