mod error;
mod requester;
mod responder;

use std::io::{Error, ErrorKind};

use bytes::BytesMut;
use futures::StreamExt;
use slog::Logger;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

// We use 2-byte size framed headers.
#[allow(dead_code)]
pub const HEADER_LEN: usize = 2;
#[allow(dead_code)]
pub const MAX_BUF_LEN: usize = 65536;

pub use error::SpdmError;

type Transport = Framed<TcpStream, LengthDelimitedCodec>;

#[allow(dead_code)]
pub fn framed_transport(sock: TcpStream) -> Transport {
    LengthDelimitedCodec::builder()
        .length_field_length(HEADER_LEN)
        .new_framed(sock)
}

pub async fn recv(
    log: &Logger,
    transport: &mut Transport,
) -> Result<BytesMut, SpdmError> {
    if let Some(rsp) = transport.next().await {
        let rsp = rsp?;
        debug!(log, "Received {:x?}", &rsp[..]);
        Ok(rsp)
    } else {
        Err(Error::new(ErrorKind::ConnectionAborted, "SPDM channel closed")
            .into())
    }
}
