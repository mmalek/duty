use crate::error::Error;
use ssh2::{Channel, Session};
use std::net::TcpStream;
use std::path::Path;

pub fn execute(
    address: &str,
    username: &str,
    command: &str,
    private_key_path: Option<&Path>,
) -> Result<Channel, Error> {
    let tcp = TcpStream::connect(address).map_err(Error::OutgoingConnectionError)?;

    let mut sess = Session::new().map_err(Error::SshConnectionError)?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(Error::SshConnectionError)?;

    let mut pkey_missing_or_failes = true;
    if let Some(pkey) = private_key_path {
        let pubkey_auth = sess.userauth_pubkey_file(username, None, &pkey, None);
        pkey_missing_or_failes = pubkey_auth.is_err();
    }

    if pkey_missing_or_failes {
        sess.userauth_agent(&username)
            .map_err(Error::SshConnectionError)?;
    }

    let mut channel = sess.channel_session().map_err(Error::SshConnectionError)?;

    channel.exec(&command).map_err(Error::SshConnectionError)?;

    Ok(channel)
}
