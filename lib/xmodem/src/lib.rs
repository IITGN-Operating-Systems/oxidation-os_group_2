#![cfg_attr(feature = "no_std", no_std)]
// #![feature(decl_macro)]

use shim::io;
use shim::ioerr;

#[cfg(test)]
mod tests;
mod read_ext;
mod progress;

pub use progress::{Progress, ProgressFn};

use read_ext::ReadExt;

const SOH: u8 = 0x01;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;

/// Implementation of the XMODEM protocol.
pub struct Xmodem<R> {
    packet: u8,
    started: bool,
    inner: R,
    progress: ProgressFn,
}

impl Xmodem<()> {
    /// Transmits data to the receiver to using the XMODEM protocol.
    /// If the length of the total data yielded by data is not a multiple
    /// of 128 bytes, the data is padded with zeroes.
    ///
    /// Returns the number of bytes written to to, excluding padding zeroes.
    #[inline]
    pub fn transmit<R, W>(data: R, to: W) -> io::Result<usize>
    where
        W: io::Read + io::Write,
        R: io::Read,
    {
        Xmodem::transmit_with_progress(data, to, progress::noop)
    }

    /// Transmits data with a progress callback.
    pub fn transmit_with_progress<R, W>(mut data: R, to: W, f: ProgressFn) -> io::Result<usize>
    where
        W: io::Read + io::Write,
        R: io::Read,
    {
        let mut transmitter = Xmodem::new_with_progress(to, f);
        // --- NEW: INITIAL HANDSHAKE ---
        // Wait for the initial NAK from the receiver.
        let initial = transmitter.read_byte(true)?;
        if initial != NAK {
            return ioerr!(InvalidData, "expected initial NAK");
        }
        transmitter.started = true;
        // --------------------------------

        let mut packet = [0u8; 128];
        let mut written = 0;
        'next_packet: loop {
            let n = data.read_max(&mut packet)?;
            // Pad remaining bytes with zeroes.
            packet[n..].iter_mut().for_each(|b| *b = 0);

            if n == 0 {
                // --- EOT HANDSHAKE (sender) ---
                // To end transmission, the sender sends:
                //   EOT, waits for NAK, then sends EOT and waits for ACK.
                transmitter.write_byte(EOT)?;
                transmitter.expect_byte(NAK, "NAK after EOT")?;
                transmitter.write_byte(EOT)?;
                transmitter.expect_byte(ACK, "ACK after second EOT")?;
                return Ok(written);
            }

            // Try sending the packet up to 10 times.
            for _ in 0..10 {
                match transmitter.write_packet(&packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(_) => {
                        written += n;
                        continue 'next_packet;
                    }
                }
            }

            return ioerr!(BrokenPipe, "bad transmit");
        }
    }

    /// Receives data using the XMODEM protocol.
    #[inline]
    pub fn receive<R, W>(from: R, into: W) -> io::Result<usize>
    where
        R: io::Read + io::Write,
        W: io::Write,
    {
        Xmodem::receive_with_progress(from, into, progress::noop)
    }

    /// Receives data with a progress callback.
    pub fn receive_with_progress<R, W>(from: R, mut into: W, f: ProgressFn) -> io::Result<usize>
    where
        R: io::Read + io::Write,
        W: io::Write,
    {
        let mut receiver = Xmodem::new_with_progress(from, f);
        // Receiver immediately sends a NAK to signal readiness.
        receiver.write_byte(NAK)?;
        let mut packet = [0u8; 128];
        let mut received = 0;
        'next_packet: loop {
            for _ in 0..10 {
                match receiver.read_packet(&mut packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(0) => break 'next_packet, // End-of-transmission.
                    Ok(n) => {
                        received += n;
                        into.write_all(&packet)?;
                        continue 'next_packet;
                    }
                }
            }
            return ioerr!(BrokenPipe, "bad receive");
        }
        Ok(received)
    }
}

/// Computes the checksum as the sum of all bytes modulo 256.
fn get_checksum(buf: &[u8]) -> u8 {
    buf.iter().fold(0, |a, b| a.wrapping_add(*b))
}

impl<T: io::Read + io::Write> Xmodem<T> {
    /// Returns a new Xmodem instance.
    pub fn new(inner: T) -> Self {
        Xmodem {
            packet: 1,
            started: false,
            inner,
            progress: progress::noop,
        }
    }

    /// Returns a new Xmodem instance with a progress callback.
    pub fn new_with_progress(inner: T, f: ProgressFn) -> Self {
        Xmodem {
            packet: 1,
            started: false,
            inner,
            progress: f,
        }
    }

    /// Reads a single byte from the inner I/O stream.
    /// If abort_on_can is true and the byte is CAN, returns a ConnectionAborted error.
    fn read_byte(&mut self, abort_on_can: bool) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;
        let byte = buf[0];
        if abort_on_can && byte == CAN {
            return ioerr!(ConnectionAborted, "received CAN");
        }
        Ok(byte)
    }

    /// Writes a single byte to the inner I/O stream.
    fn write_byte(&mut self, byte: u8) -> io::Result<()> {
        self.inner.write_all(&[byte])
    }

    /// Reads a byte and compares it to byte. On mismatch, sends a CAN and returns an error.
    fn expect_byte_or_cancel(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        let b = self.read_byte(false)?;
        if b != byte {
            self.write_byte(CAN)?;
            if b == CAN {
                return ioerr!(ConnectionAborted, "received CAN");
            } else {
                return ioerr!(InvalidData, expected);
            }
        }
        Ok(b)
    }

    /// Reads a byte and verifies that it matches byte.
    /// For CAN, does not abort on receiving CAN.
    fn expect_byte(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        let b = if byte == CAN { self.read_byte(false)? } else { self.read_byte(true)? };
        if b != byte {
            self.write_byte(CAN)?;
            return ioerr!(InvalidData, expected);
        }
        Ok(b)
    }

    /// Reads (downloads) a single packet (128 bytes) from the inner stream.
    /// If the provided buffer is too small, returns UnexpectedEof.
    /// On receiving EOT, performs the handshake and returns 0.
    /// Otherwise, verifies the packet number, its complement, and checksum.
    pub fn read_packet(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Ensure buffer is large enough.
        if buf.len() < 128 {
            return ioerr!(UnexpectedEof, "buffer too small for packet");
        }
        // Read header byte.
        let first = self.read_byte(true)?;
        if first == EOT {
            // EOT handshake.
            self.write_byte(NAK)?;
            self.expect_byte(EOT, "second EOT")?;
            self.write_byte(ACK)?;
            return Ok(0);
        }
        // If header is not SOH, read one extra byte to decide the error kind.
        if first != SOH {
            let second = self.read_byte(false)?;
            self.write_byte(CAN)?;
            if second == CAN {
                return ioerr!(ConnectionAborted, "received CAN");
            } else {
                return ioerr!(InvalidData, "expected SOH or EOT");
            }
        }
        // Read packet number and its complement.
        let pkt_num = self.read_byte(true)?;
        let pkt_num_comp = self.read_byte(true)?;
        if pkt_num != self.packet || pkt_num_comp != (255 - self.packet) {
            self.write_byte(CAN)?;
            return ioerr!(InvalidData, "invalid packet number");
        }
        // Read 128 bytes of packet data.
        self.inner.read_exact(buf)?;
        let checksum = get_checksum(buf);
        let transmitted = self.read_byte(false)?;
        if checksum != transmitted {
            self.write_byte(NAK)?;
            return ioerr!(Interrupted, "checksum mismatch");
        }
        // Packet received correctly: send ACK, update packet number, and report progress.
        self.write_byte(ACK)?;
        self.packet = self.packet.wrapping_add(1);
        (self.progress)(Progress::Packet(self.packet));
        Ok(128)
    }

    /// Sends (uploads) a single packet to the inner stream.
    /// If buf is empty, performs the EOT handshake.
    /// Otherwise, sends SOH, packet number, its complement, 128-byte data, and checksum,
    /// then waits for the receiver's response.
    pub fn write_packet(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            // EOT handshake:
            if !self.started {
                // If the transmission hasn’t started yet,
                // first consume the initial NAK from the receiver.
                self.expect_byte(NAK, "initial NAK")?;
                self.started = true;
            }
            self.write_byte(EOT)?;
            self.expect_byte(NAK, "NAK after EOT")?;
            self.write_byte(EOT)?;
            self.expect_byte(ACK, "ACK after second EOT")?;
            return Ok(0);
        } else {
            // Data packet transmission (unchanged) …
            self.write_byte(SOH)?;
            self.write_byte(self.packet)?;
            self.write_byte(255 - self.packet)?;
            self.inner.write_all(buf)?;
            let checksum = get_checksum(buf);
            self.write_byte(checksum)?;

            // Wait for receiver response.
            let response = self.read_byte(true)?;
            if response == NAK {
                return ioerr!(Interrupted, "checksum mismatch, retransmit packet");
            }
            if response != ACK {
                return ioerr!(InvalidData, "invalid response from receiver");
            }
            self.packet = self.packet.wrapping_add(1);
            (self.progress)(Progress::Packet(self.packet));
            Ok(buf.len())
        }
    }


    /// Flushes the inner I/O stream.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
