use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot start server")]
    ServerFailedToStart(#[source] io::Error),
    #[error("message header deserialization failed")]
    MsgHeaderDeserFailed(#[source] bincode::Error),
    #[error("message body deserialization failed")]
    MsgBoodyDeserFailed(#[source] bincode::Error),
    #[error("cannot read message raw data")]
    CannotReadMsgRawData(#[source] io::Error),
    #[error("message header serialization failed")]
    MsgHeaderSerFailed(#[source] bincode::Error),
    #[error("message body serialization failed")]
    MsgBoodySerFailed(#[source] bincode::Error),
    #[error("incoming connection error")]
    IncomingConnectionError(#[source] io::Error),
    #[error("outgoing connection error")]
    OutgoingConnectionError(#[source] io::Error),
    #[error("SSH connection error")]
    SshConnectionError(#[source] ssh2::Error),
}
