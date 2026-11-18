//! Native messaging framing: a 32-bit length in native byte order, then that
//! many bytes of UTF-8 JSON. The same framing is used on the app's socket.

use std::io::{self, Read, Write};

/// Browsers refuse host-to-browser messages above 1 MB; nothing we send in
/// either direction comes close, so the same cap applies both ways.
pub const MAX_FRAME: usize = 1024 * 1024;

/// Reads one frame. `Ok(None)` means the other side closed cleanly between frames.
pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_ne_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    writer.write_all(&(payload.len() as u32).to_ne_bytes())?;
    writer.write_all(payload)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_frames_and_reports_a_clean_end() {
        let mut wire = Vec::new();
        write_frame(&mut wire, br#"{"a":1}"#).unwrap();
        write_frame(&mut wire, b"").unwrap();
        let mut reader = wire.as_slice();
        assert_eq!(read_frame(&mut reader).unwrap().unwrap(), br#"{"a":1}"#);
        assert_eq!(read_frame(&mut reader).unwrap().unwrap(), b"");
        assert!(read_frame(&mut reader).unwrap().is_none());
    }

    #[test]
    fn refuses_an_oversized_or_truncated_frame() {
        let huge = ((MAX_FRAME + 1) as u32).to_ne_bytes();
        assert!(read_frame(&mut huge.as_slice()).is_err());

        let mut cut = 10u32.to_ne_bytes().to_vec();
        cut.extend_from_slice(b"abc");
        assert!(read_frame(&mut cut.as_slice()).is_err());
    }
}
