use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("message deserialization failed: {0}")]
    MsgDeserFailed(String),
    #[error("message serialization {0}")]
    MsgSerFailed(String),
    #[error("incoming connection error")]
    IncomingConnectionError(#[source] io::Error),
    #[error("outgoing connection error")]
    OutgoingConnectionError(#[source] io::Error),
    #[error("SSH connection error")]
    SshConnectionError(#[source] ssh2::Error),
}
