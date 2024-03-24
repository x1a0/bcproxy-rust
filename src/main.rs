use codec::{BatMudCodec, BatMudFrame};
use tokio::net::tcp::WriteHalf;
use tokio::net::TcpStream;
use tokio::{io::AsyncWriteExt, net::tcp::ReadHalf};
use tokio_stream::StreamExt as _;
use tokio_util::codec::{BytesCodec, FramedRead};

mod codec;
mod color;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7788").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = process(stream).await {
                tracing::error!("an error occurred; error = {:?}", e);
            }
        });
    }
}

async fn process(mut client: TcpStream) -> Result<(), std::io::Error> {
    let mut server = TcpStream::connect("batmud.bat.org:2023").await?;
    let bc_mode = "\x1bbc 1\n".as_bytes();
    server.write_all(bc_mode).await?;

    let (client_reader, client_writer) = client.split();
    let (server_reader, server_writer) = server.split();

    let server_to_client = server_to_client(server_reader, client_writer);
    let client_to_server = client_to_server(client_reader, server_writer);

    tokio::try_join!(server_to_client, client_to_server)?;

    Ok(())
}

async fn client_to_server<'a>(
    reader: ReadHalf<'a>,
    mut writer: WriteHalf<'a>,
) -> Result<(), std::io::Error> {
    let mut transport = FramedRead::new(reader, BytesCodec::new());

    while let Some(line) = transport.next().await {
        match line {
            Ok(line) => {
                writer.write_all(&line).await?;
            }
            Err(e) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        }
    }

    Ok(())
}

async fn server_to_client<'a>(
    reader: ReadHalf<'a>,
    mut writer: WriteHalf<'a>,
) -> Result<(), std::io::Error> {
    let mut transport = FramedRead::new(reader, BatMudCodec::new());

    while let Some(frame) = transport.next().await {
        match frame {
            Ok(BatMudFrame::Text(line)) => {
                writer.write_all(&line).await?;
            }

            Ok(BatMudFrame::Code(control_code)) => {
                let bytes = control_code.to_bytes();
                writer.write_all(&bytes).await?;
                tracing::debug!(
                    "proxying control code as: {}",
                    String::from_utf8(bytes).unwrap()
                );
            }

            Ok(BatMudFrame::Continue) => {}

            Err(e) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        }
    }

    Ok(())
}
