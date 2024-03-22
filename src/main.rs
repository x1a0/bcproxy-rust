use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;

mod io;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7788").await?;

    while let Ok((mut inbound, _)) = listener.accept().await {
        let mut outbound = TcpStream::connect("batmud.bat.org:2023").await?;

        tokio::spawn(async move {
            let result = io::proxy_bidirection(&mut inbound, &mut outbound).await;
            match result {
                Err(e) => {
                    eprintln!("failed to copy: {}", e);
                }
                Ok((x, y)) => {
                    println!("{} bytes copied from a to b", x);
                    println!("{} bytes copied from b to a", y);
                }
            }
        });
    }

    Ok(())
}
