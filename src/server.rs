use std::{net::TcpListener, thread::spawn};

use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    Message,
};

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

            loop {
                let msg = websocket.read().unwrap();

                match msg {
                    Message::Binary(v) if v.as_slice() == [1] => {
                        println!("Recieved: Sync Request");
                        websocket
                            .send(Message::Text("Sync Request approved".to_string()))
                            .unwrap();
                    }
                    Message::Binary(v) if v.as_slice() == [2] => {
                        println!("Recieved: Sync Completed");
                        websocket.send(Message::Text("OK".to_string())).unwrap();
                    }
                    Message::Text(_) => websocket.send(msg).unwrap(),
                    _ => println!("Unrecognized request received"),
                };
            }
        });
    }
}
