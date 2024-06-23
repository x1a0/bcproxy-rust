use bytes::Bytes;
use codec::{BatMudCodec, BatMudFrame};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tokio::net::tcp::WriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::{io::AsyncWriteExt, net::tcp::ReadHalf};
use tokio_stream::StreamExt as _;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::codec::control_code::mapper::Mapper;

mod codec;
mod color;

enum DbMessage {
    Mapper(Mapper),
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let db_address = std::env::vars()
        .find(|(key, _)| key == "BCP_DB_HOST")
        .map(|(_, value)| value)
        .unwrap_or("localhost".to_string());

    let db_port = std::env::vars()
        .find(|(key, _)| key == "BCP_DB_PORT")
        .map(|(_, value)| value)
        .unwrap_or("5432".to_string());

    let db_user = std::env::vars()
        .find(|(key, _)| key == "BCP_DB_USER")
        .map(|(_, value)| value)
        .unwrap_or("batmud".to_string());

    let db_password = std::env::vars()
        .find(|(key, _)| key == "BCP_DB_PASSWORD")
        .map(|(_, value)| value)
        .unwrap_or("batmud".to_string());

    let db_name = std::env::vars()
        .find(|(key, _)| key == "BCP_DB_NAME")
        .map(|(_, value)| value)
        .unwrap_or("batmud".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                db_user, db_password, db_address, db_port, db_name
            )
            .as_str(),
        )
        .await
        .expect("could not connect to database");

    let (tx, rx) = mpsc::channel::<DbMessage>(64);
    tokio::spawn(handle_db_message(rx, pool));

    let port = std::env::vars()
        .find(|(key, _)| key == "BCP_PORT")
        .map(|(_, value)| value)
        .unwrap_or("7788".to_string());

    tracing::info!("Starting BCP server on port {}", port);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = process(stream, tx).await {
                tracing::error!("an error occurred; error = {:?}", e);
            }
        });
    }
}

async fn process(mut client: TcpStream, tx: Sender<DbMessage>) -> Result<(), std::io::Error> {
    let remote_addr = std::env::vars()
        .find(|(key, _)| key == "REMOTE_ADDR")
        .map(|(_, value)| value)
        .unwrap_or("batmud.bat.org:2023".to_string());

    tracing::info!("Connecting to remote server at {}", remote_addr);

    let mut server = TcpStream::connect(remote_addr).await?;
    let bc_mode = "\x1bbc 1\n".as_bytes();
    server.write_all(bc_mode).await?;

    let (client_reader, client_writer) = client.split();
    let (server_reader, server_writer) = server.split();

    let server_to_client = server_to_client(server_reader, client_writer, &tx);
    let client_to_server = client_to_server(client_reader, server_writer, &tx);

    tokio::try_join!(server_to_client, client_to_server)?;

    Ok(())
}

async fn client_to_server<'a>(
    reader: ReadHalf<'a>,
    mut writer: WriteHalf<'a>,
    tx: &Sender<DbMessage>,
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
    tx: &Sender<DbMessage>,
) -> Result<(), std::io::Error> {
    let mut transport = FramedRead::new(reader, BatMudCodec::new());

    while let Some(frame) = transport.next().await {
        match frame {
            Ok(BatMudFrame::Text(line)) => {
                writer.write_all(&line).await?;
            }

            Ok(BatMudFrame::Code(control_code)) => {
                let bytes = control_code.to_bytes();

                if control_code.is_mapper() {
                    let mapper: Mapper =
                        Mapper::try_from(control_code.get_children_bytes().as_slice())?;
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(DbMessage::Mapper(mapper)).await;
                    });
                }

                writer.write_all(&bytes).await?;
                tracing::debug!("proxying control code as: {:?}", Bytes::from(bytes));
            }

            Ok(BatMudFrame::Continue) => {}

            Err(e) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        }
    }

    Ok(())
}

async fn handle_db_message(
    mut rx: Receiver<DbMessage>,
    pool: Pool<Postgres>,
) -> Result<(), std::io::Error> {
    while let Some(message) = rx.recv().await {
        match message {
            DbMessage::Mapper(mapper) => {
                match upsert_room(pool.clone(), mapper).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("could not upsert room: {:?}", e);
                    }
                };
            }
        }
    }

    Ok(())
}

async fn upsert_room(pool: Pool<Postgres>, mapper: Mapper) -> Result<(), sqlx::Error> {
    if let Mapper::Area {
        room_id,
        room_name,
        area_name,
        room_description,
        indoor,
        exits,
        from,
    } = mapper
    {
        sqlx::query(
            r##"
                INSERT INTO rooms (
                    id,
                    area,
                    name,
                    description,
                    indoor,
                    exits,
                    from_dir
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (id) DO NOTHING"##,
        )
        .bind(room_id)
        .bind(area_name)
        .bind(room_name)
        .bind(room_description)
        .bind(indoor)
        .bind(exits)
        .bind(from)
        .execute(&pool)
        .await?;
    }

    Ok(())
}
