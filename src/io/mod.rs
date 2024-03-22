mod proxy;

use std::{
    future::poll_fn,
    pin::Pin,
    task::{ready, Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite};

use self::proxy::ProxyBuffer;

enum ProxyState {
    Running(ProxyBuffer),
    ShuttingDown(u64),
    Done(u64),
}

pub async fn proxy_bidirection<A, B>(
    server: &mut A,
    client: &mut B,
) -> Result<(u64, u64), std::io::Error>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut inbound = ProxyState::Running(ProxyBuffer::new());
    let mut outbound = ProxyState::Running(ProxyBuffer::new());
    poll_fn(|cx| {
        let inbound = server_to_client(cx, &mut inbound, server, client)?;
        let outbound = client_to_server(cx, &mut outbound, client, server)?;

        // It is not a problem if ready! returns early because transfer_one_direction for the
        // other direction will keep returning TransferState::Done(count) in future calls to poll
        let inbound = ready!(inbound);
        let outbound = ready!(outbound);

        Poll::Ready(Ok((inbound, outbound)))
    })
    .await
}

fn server_to_client<R, W>(
    cx: &mut Context<'_>,
    state: &mut ProxyState,
    server: &mut R,
    client: &mut W,
) -> Poll<std::io::Result<u64>>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut server = Pin::new(server);
    let mut client = Pin::new(client);

    loop {
        match state {
            ProxyState::Running(buf) => {
                let count = ready!(buf.poll_copy(cx, server.as_mut(), client.as_mut()))?;
                *state = ProxyState::ShuttingDown(count);
            }
            ProxyState::ShuttingDown(count) => {
                ready!(client.as_mut().poll_shutdown(cx))?;

                *state = ProxyState::Done(*count);
            }
            ProxyState::Done(count) => return Poll::Ready(Ok(*count)),
        }
    }
}

fn client_to_server<R, W>(
    cx: &mut Context<'_>,
    state: &mut ProxyState,
    client: &mut R,
    server: &mut W,
) -> Poll<std::io::Result<u64>>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut server = Pin::new(server);
    let mut client = Pin::new(client);

    loop {
        match state {
            ProxyState::Running(buf) => {
                let count = ready!(buf.poll_copy(cx, client.as_mut(), server.as_mut()))?;
                *state = ProxyState::ShuttingDown(count);
            }
            ProxyState::ShuttingDown(count) => {
                ready!(server.as_mut().poll_shutdown(cx))?;

                *state = ProxyState::Done(*count);
            }
            ProxyState::Done(count) => return Poll::Ready(Ok(*count)),
        }
    }
}
