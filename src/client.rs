use std::io::{self, Write};
use std::{net::TcpStream, thread::sleep, time::Duration};
use tungstenite::{connect, stream::MaybeTlsStream, util::NonBlockingError, Message, WebSocket};
use url::Url;

fn lorenz_attractor(
    x: f64,
    y: f64,
    z: f64,
    sigma: f64,
    rho: f64,
    beta: f64,
    h: f64,
) -> (f64, f64, f64) {
    let new_x = x + (sigma * (y - x)) * h;
    let new_y = y + (x * (rho - z) - y) * h;
    let new_z = z + (x * y - beta * z) * h;

    (new_x, new_y, new_z)
}

fn send_request(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    name: &str,
    request_id: u8,
) -> () {
    socket
        .send(Message::Binary(vec![request_id]))
        .expect(format!("Unable to send request: {}", name).as_str());

    println!("Sent: {}", name);
}

fn receive_msg(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> () {
    let msg = socket.read().expect("Error reading message");
    println!("Recieved: {}", msg);
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
    let mut syncing = false;

    let mut state = lorenz_attractor(seed.0, seed.1, seed.2, sigma, rho, beta, h);
    println!("x = {}, y = {}, z = {}", state.0, state.1, state.2);

    let mut input = String::new();
    print!("Type a message you want to encrypt: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_string();

    send_request(&mut socket, "Sync Request", 1);
    receive_msg(&mut socket);

    loop {
        state = lorenz_attractor(state.0, state.1, state.2, sigma, rho, beta, h);
        socket
            .send(Message::Binary(state.0.to_ne_bytes().to_vec()))
            .expect("oopsie");
        socket
            .send(Message::Binary(state.1.to_ne_bytes().to_vec()))
            .expect("oopsie");
        socket
            .send(Message::Binary(state.2.to_ne_bytes().to_vec()))
            .expect("oopsie");

        println!("x = {}, y = {}, z = {}", state.0, state.1, state.2);

        // Allows for a new message to be created, making the sync better, but
        // slower
        sleep(Duration::new(0, 5000000));
    }
}
