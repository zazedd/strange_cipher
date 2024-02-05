use serial_test::serial;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use std::sync::{Arc, Mutex};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Child, ChildStdout},
    };

    use rand::{
        distributions::{Alphanumeric, DistString},
        Rng,
    };

    #[test]
    #[serial] // These tests need to be ran one after the other
    fn non_concurrent() {
        let (mut server_handle, reader) = setup_server();

        let mut sent_messages = Vec::new();
        let decoded_messages = Arc::new(Mutex::new(Vec::new()));

        let decoded_messages_clone = Arc::clone(&decoded_messages);
        thread::spawn(move || {
            for line in reader.lines() {
                let line = line.expect("Failed to read line from server stdout");
                if line.contains("Decoded message: ") {
                    let decoded_message = line.replace("Decoded message: ", "");
                    decoded_messages_clone.lock().unwrap().push(decoded_message);
                }
            }
        });

        thread::sleep(Duration::from_secs(1));

        for _ in 0..100 {
            let random_message = Alphanumeric.sample_string(
                &mut rand::thread_rng(),
                rand::thread_rng().gen_range(10..4096),
            );

            sent_messages.push(random_message.clone());

            let client_thread = thread::spawn(move || run_client(random_message));

            client_thread.join().expect("Couldn't join thread");
        }

        server_handle.kill().expect("Failed to kill the server");
        server_handle.wait().expect("Failed to wait for the server");

        let decoded_messages = decoded_messages.lock().unwrap();
        for (sent_message, decoded_message) in sent_messages.iter().zip(decoded_messages.iter()) {
            assert_eq!(sent_message, decoded_message);
        }
    }

    #[test]
    #[serial]
    fn concurrent() {
        let (mut server_handle, reader) = setup_server();

        let mut sent_messages = Vec::new();
        let decoded_messages = Arc::new(Mutex::new(Vec::new()));

        let decoded_messages_clone = Arc::clone(&decoded_messages);
        thread::spawn(move || {
            for line in reader.lines() {
                let line = line.expect("Failed to read line from server stdout");
                if line.contains("Decoded message: ") {
                    let decoded_message = line.replace("Decoded message: ", "");
                    decoded_messages_clone.lock().unwrap().push(decoded_message);
                }
            }
        });

        thread::sleep(Duration::from_secs(1));

        let handles: Vec<_> = (0..100)
            .map(|_| {
                let random_message = Alphanumeric.sample_string(
                    &mut rand::thread_rng(),
                    rand::thread_rng().gen_range(10..4096),
                );

                sent_messages.push(random_message.clone());

                thread::spawn(move || run_client(random_message))
            })
            .collect();

        for handle in handles {
            handle.join().expect("Couldn't join thread");
        }

        server_handle.kill().expect("Failed to kill the server");
        server_handle.wait().expect("Failed to wait for the server");

        let mut decoded_messages = decoded_messages.lock().unwrap();
        assert_eq!(sent_messages.sort(), decoded_messages.sort());
    }

    fn setup_server() -> (Child, BufReader<ChildStdout>) {
        let mut server_handle = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("server")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start the server");

        let stdout = server_handle
            .stdout
            .take()
            .expect("Failed to capture server stdout");

        let reader = BufReader::new(stdout);

        (server_handle, reader)
    }

    fn run_client(random_message: String) -> () {
        let mut client_process = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("client")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start the client");

        if let Some(mut stdin) = client_process.stdin.take() {
            stdin
                .write(random_message.as_bytes())
                .expect("Failed to write to stdin");
        }

        let client_status = client_process
            .wait()
            .expect("Failed to wait for the client");

        assert!(client_status.success());
    }
}
