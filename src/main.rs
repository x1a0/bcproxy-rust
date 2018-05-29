#[macro_use] extern crate log;
extern crate env_logger;
extern crate tokio;

use std::sync::{Arc, Mutex};
use std::env;
use std::net::{SocketAddr, Shutdown};
use std::io::{self, Read, Write};

use tokio::io::{copy, shutdown};
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

fn main() {
    env_logger::init();

    let listen_addr = env::args().nth(1).unwrap_or("127.0.0.1:9999".to_string());
    let listen_addr = listen_addr.parse::<SocketAddr>().unwrap();

    let bat_addr = env::args().nth(2).unwrap_or("83.145.249.153:2023".to_string());
    let bat_addr = bat_addr.parse::<SocketAddr>().unwrap();

    // Listen for incoming connections.
    let socket = TcpListener::bind(&listen_addr).unwrap();
    info!("Listening on: {}", listen_addr);

    let done = socket.incoming()
        .map_err(|e| error!("Error accepting socket; error = {}", e))
        .for_each(move |client| {
            let bat = TcpStream::connect(&bat_addr);
            info!("Connecting to: {}", bat_addr);

            let amounts = bat.and_then(move |bat| {
                let client_reader = ProxyTcpStream(Arc::new(Mutex::new(client)));
                let client_writer = client_reader.clone();
                let bat_reader = ProxyTcpStream(Arc::new(Mutex::new(bat)));
                let bat_writer = bat_reader.clone();

                let client_to_bat = copy(client_reader, bat_writer)
                    .and_then(|(n, _, bat_writer)| {
                        shutdown(bat_writer).map(move |_| n)
                    });

                let bat_to_client = copy(bat_reader, client_writer)
                    .and_then(|(n, _, client_writer)| {
                        shutdown(client_writer).map(move |_| n)
                    });

                client_to_bat.join(bat_to_client)
            });

            let msg = amounts.map(|(from_client, from_bat)| {
                info!("Client wrote {} bytes and received {} bytes", from_client, from_bat);
            }).map_err(|e| {
                // Don't panic. Maybe the client just disconnected too soon.
                error!("Error: {}", e);
            });

            tokio::spawn(msg);

            Ok(())
        });

    tokio::run(done);
}

#[derive(Clone)]
struct ProxyTcpStream(Arc<Mutex<TcpStream>>);

impl Read for ProxyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for ProxyTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncRead for ProxyTcpStream {}

impl AsyncWrite for ProxyTcpStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        try!(self.0.lock().unwrap().shutdown(Shutdown::Write));
        Ok(().into())
    }
}
