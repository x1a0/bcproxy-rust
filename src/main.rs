use crate::codec::{ServerCodec, ClientCodec};

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};
use futures::future::FutureExt;
use futures::{StreamExt, SinkExt};
use native_tls::TlsConnector;
use tokio_util::codec::Framed;

use std::env;
use std::error::Error;
use std::net::ToSocketAddrs;

mod bat_tag;
mod codec;
mod color;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:9999".to_string());

    let bat_server_addr = "bat.org:2022".to_string();

    log::info!("Proxy listening on: {}", listen_addr);
    log::info!("Proxying to: {}", bat_server_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let proxy = proxy(inbound, bat_server_addr.clone()).map(|r| {
            if let Err(e) = r {
                log::error!("Failed to proxy; error={}", e);
            }
        });

        tokio::spawn(proxy);
    }

    Ok(())
}

async fn proxy(inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    log::info!("Start proxying");

    let addr = proxy_addr.to_socket_addrs()?.next()
        .ok_or("Failed to resolve bat.org")?;
    let socekt = TcpStream::connect(&addr).await?;
    let cx = TlsConnector::builder().build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);
    let outbound = cx.connect("bat.org", socekt).await?;

    let codec = ClientCodec::new();
    let inbound = Framed::new(inbound, codec);
    let (mut client_sink, mut client_stream) = inbound.split();

    let codec = ServerCodec::new();
    let outbound = Framed::new(outbound, codec);
    let (mut server_sink, mut server_stream) = outbound.split();
    
    // Send "<ECS>bc 1\n" to enable BC mode.
    // TODO: send after login?
    //let enable_bc_mode = Bytes::from("\x1bbc 1\n");
    //server_sink.send(enable_bc_mode).await?;
    //server_sink.flush().await?;

    let client_to_server = async {
        server_sink.send_all(&mut client_stream).await?;
        server_sink.close().await
    };

    let server_to_client = async {
        client_sink.send_all(&mut server_stream).await?;
        client_sink.close().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
