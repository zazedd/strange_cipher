use std::{
    io::{self, Write},
    net::TcpStream,
};
use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

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

fn recieve_msg(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> () {
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

    send_request(&mut socket, "Sync Request", 1);
    recieve_msg(&mut socket);

    send_request(&mut socket, "Testing", 2);
    recieve_msg(&mut socket);

    loop {
        let mut input = String::new();
        print!("Type a message: ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim().to_string();

        socket.send(Message::Text(input.clone())).unwrap();

        println!("Sent: {}", input);

        recieve_msg(&mut socket);
    }
}
