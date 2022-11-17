use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Receiver, RecvError, SendError, Sender};

pub struct MpscStream {
    sender: Sender<u8>,
    receiver: Receiver<u8>,
}

impl MpscStream {
    pub fn new_pair() -> (MpscStream, MpscStream) {
        let (send1, recv1) = channel();
        let (send2, recv2) = channel();
        (
            MpscStream {
                sender: send1,
                receiver: recv2,
            },
            MpscStream {
                sender: send2,
                receiver: recv1,
            },
        )
    }
}

impl Read for MpscStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for b in buf.iter_mut() {
            match self.receiver.recv() {
                Ok(data) => *b = data,
                Err(RecvError) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Cannot receive data: channel disconnected",
                    ))
                }
            }
        }

        Ok(buf.len())
    }
}

impl Write for MpscStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for b in buf.iter() {
            match self.sender.send(*b) {
                Ok(()) => {}
                Err(SendError(_)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Cannot send data: channel disconnected",
                    ))
                }
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
