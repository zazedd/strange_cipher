# Strange Cipher
An encryption/decryption algorithm based on Lorenz's Strange Attractor, and WebSockets for communication.

## How it Works

There is a Lorenz Strange Attractor running on the server for each client, and one on the client side as well.  
The Attractors have the same pre-conditions, but each start at different positions in space, resulting in vastly different trajectories.
The basic idea is:  
```
Attractors with different trajectories -> sync them -> create a stream cipher on the server and client -> encrypt with client cipher -> send encrypted message -> decrypt with server cipher -> unsync the Attractors
```

The stream cipher is constructed with the current `y` coordinate of the Attractor at each frame. Because the attractors are synced, the `y` coordinates should be the same, so the server is able to decrypt the message.

But how can we sync these seemingly chaotic systems?

### Syncing Process

Steven Strogatz described, in 2003, an easy way of syncing two or more chaotic systems:
- Take one of the Systems, the `Driver`, that will transmit its current state in a one-way communication tunnel.
- The other system becomes the `Reciever`.
- If we force the `Reciever`'s `x` coordinate to be equal to the `x` coordinate from the `Driver` system, we observe that, after a small number of iterations, the systems become synced

![sync](.github/sync.git)

In this example, the bottom Attractor, the `Reciever`, struggles to display the normal Butterfly-like behaviour at first, but then, after a few seconds, for each new point the other coordinates start coming
closer and closer to the `Driver` Attractor, until they are dancing in perfect sync with their own doppelgänger, in Steven Strogatz’s words.


### Why are Chaotic Attractors good for Cryptography?

They are **Deterministic**, meaning that, given the same pre-conditions, the outcome will always be the same.
They are also very sensitive to those pre-conditions, any little change means a **huge** difference in the outcome, which one of the why they are called chaotic (the other is that it is hard to predict what will happen next).

## Running

Run the server:
```bash
cargo run --bin server
```

And the client:
```bash
cargo run --bin client
```

in separate terminal windows, write something on the client, and watch it get encoded on the client and decoded on the server.

## Testing

Run the tests with the command:
```bash
cargo test
```

The testing suite is comprised of:
- [ ] Unit Tests
  - [x] Encryption function
  - [x] Dencryption function
  - [ ] Lorenz Attractor Syncing

- [x] Integration Tests
  - [x] 100 Non-Concurrent Clients
  - [x] 100 Concurrent Clients

## Security Considerations

Please note that I did not formally prove this algorithm and it may not be suitable for real-world applications.  
It may contain security concerns and/or not be 100% accurate all of the time.

## Credits

- **Syncing GIF**
  - **Author:** Iaocopo Garizio
  - **URL:** [syncgif](https://iacopogarizio.com/projects/synchronizing-lorenz-attractors-i)

