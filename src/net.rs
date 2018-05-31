use std::sync::{Arc, Mutex};
use std::net::Shutdown;
use std::io::{self, Read, Write};

use tokio::net::TcpStream;
use tokio::prelude::*;

#[derive(Clone)]
pub struct ProxyTcpStream(pub Arc<Mutex<TcpStream>>);

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
