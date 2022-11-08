use duty::transport::Bincode;
use ssh2::{Channel, Session};
use std::error::Error;
use std::net::TcpStream;
use std::path::Path;

mod ttv_calc;
use ttv_calc::TtvCalcClient;

fn main() -> Result<(), Box<dyn Error>> {
    let (_sess, channel) = execute("myserver", "mmalek", "local_worker", None)?;

    let client = TtvCalcClient::new(Bincode::new(channel))?;

    let sum = client.ttv_calc(0, 42)?;

    println!("{:?}", sum);

    Ok(())
}

pub fn execute(
    address: &str,
    username: &str,
    command: &str,
    private_key_path: Option<&Path>,
) -> Result<(Session, Channel), Box<dyn Error>> {
    let tcp = TcpStream::connect(address)?;

    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    let mut pkey_missing_or_failes = true;
    if let Some(pkey) = private_key_path {
        let pubkey_auth = sess.userauth_pubkey_file(username, None, &pkey, None);
        pkey_missing_or_failes = pubkey_auth.is_err();
    }

    if pkey_missing_or_failes {
        sess.userauth_agent(&username)?;
    }

    let mut channel = sess.channel_session()?;

    channel.exec(&command)?;

    Ok((sess, channel))
}
