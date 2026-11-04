//! Nonblocking, length-framed TCP with bounded buffers and explicit backpressure.
use serde::{de::DeserializeOwned, Serialize};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_QUEUE: usize = 16 * 1024 * 1024;
const READ_BUDGET: usize = 256 * 1024;

pub struct Socket {
    stream: TcpStream,
    incoming: Vec<u8>,
    outgoing: VecDeque<Vec<u8>>,
    offset: usize,
    queued: usize,
    last_received: Instant,
    closed: bool,
}

impl Socket {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            incoming: Vec::new(),
            outgoing: VecDeque::new(),
            offset: 0,
            queued: 0,
            last_received: Instant::now(),
            closed: false,
        })
    }

    pub fn send(&mut self, message: &impl Serialize) -> io::Result<()> {
        let bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
        if bytes.len() > MAX_FRAME || self.queued + bytes.len() + 4 > MAX_QUEUE {
            return Err(io::Error::other("multiplayer send queue exceeded"));
        }
        let mut frame = Vec::with_capacity(bytes.len() + 4);
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend(bytes);
        self.queued += frame.len();
        self.outgoing.push_back(frame);
        self.flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let mut budget = READ_BUDGET;
        while let Some(frame) = self.outgoing.front() {
            if budget == 0 {
                break;
            }
            let end = frame.len().min(self.offset + budget);
            match self.stream.write(&frame[self.offset..end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.offset += n;
                    budget -= n;
                    self.queued -= n;
                    if self.offset == frame.len() {
                        self.outgoing.pop_front();
                        self.offset = 0;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn receive<T: DeserializeOwned>(&mut self) -> io::Result<Vec<T>> {
        if self.closed && self.incoming.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.flush()?;
        let mut buffer = [0u8; 8192];
        let mut budget = READ_BUDGET;
        while budget > 0 && !self.closed {
            match self.stream.read(&mut buffer[..budget.min(8192)]) {
                Ok(0) => {
                    self.closed = true;
                    break;
                }
                Ok(n) => {
                    self.incoming.extend_from_slice(&buffer[..n]);
                    self.last_received = Instant::now();
                    budget -= n;
                    if self.incoming.len() > MAX_FRAME + READ_BUDGET {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        let mut messages = Vec::new();
        let mut offset = 0;
        while self.incoming.len() >= offset + 4 {
            let length =
                u32::from_le_bytes(self.incoming[offset..offset + 4].try_into().unwrap()) as usize;
            if length == 0 || length > MAX_FRAME {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if self.incoming.len() < offset + 4 + length {
                break;
            }
            if messages.len() >= 256 {
                return Err(io::Error::other("too many multiplayer messages"));
            }
            messages.push(
                serde_json::from_slice(&self.incoming[offset + 4..offset + 4 + length])
                    .map_err(io::Error::other)?,
            );
            offset += 4 + length;
        }
        self.incoming.drain(..offset);
        if self.closed && messages.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if self.last_received.elapsed() > Duration::from_secs(20) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(messages)
    }

    pub fn idle(&self) -> bool {
        self.outgoing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn tcp_delivers_framed_messages_in_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client =
            Socket::new(TcpStream::connect(listener.local_addr().unwrap()).unwrap()).unwrap();
        let mut server = Socket::new(listener.accept().unwrap().0).unwrap();
        client.send(&vec![1u32, 2, 3]).unwrap();
        client.send(&vec![4u32]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut received = Vec::<Vec<u32>>::new();
        while received.len() < 2 && Instant::now() < deadline {
            client.flush().unwrap();
            received.extend(server.receive::<Vec<u32>>().unwrap());
            std::thread::yield_now();
        }
        assert_eq!(received, [vec![1, 2, 3], vec![4]]);
    }

    #[test]
    fn rejects_oversized_length_before_allocating_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut server = Socket::new(listener.accept().unwrap().0).unwrap();
        stream.write_all(&u32::MAX.to_le_bytes()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match server.receive::<u32>() {
                Err(e) => {
                    assert_eq!(e.kind(), io::ErrorKind::InvalidData);
                    break;
                }
                Ok(_) => assert!(Instant::now() < deadline),
            }
            std::thread::yield_now();
        }
    }
}
