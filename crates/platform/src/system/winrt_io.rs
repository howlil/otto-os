use std::time::Instant;

use windows::Storage::Streams::{Buffer, DataReader, IInputStream, InputStreamOptions};

const READ_CHUNK_BYTES: u32 = 16 * 1024;
const DOWNLOAD_CHUNK_BYTES: u32 = 64 * 1024;

pub(crate) struct MtaResult<T>(pub(crate) T);

unsafe impl<T> Send for MtaResult<T> {}

macro_rules! await_until {
    ($operation:expr, $deadline:expr) => {{
        match $operation {
            Ok(operation) => {
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                let registered = operation.when(move |result| {
                    let _ = sender.send(result.map($crate::winrt_io::MtaResult));
                });
                let remaining = $deadline.saturating_duration_since(std::time::Instant::now());
                match registered.map(|()| receiver.recv_timeout(remaining)) {
                    Ok(Ok(Ok(value))) => Some(value.0),
                    Ok(Err(_)) => {
                        let _ = operation.Cancel();
                        None
                    }
                    _ => None,
                }
            }
            Err(_) => None,
        }
    }};
}

pub(crate) use await_until;

#[derive(Debug, Default)]
pub(crate) struct BoundedBody {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedBody {
    pub(crate) const fn with_limit(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.limit - self.bytes.len()
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) -> bool {
        if chunk.len() > self.remaining() {
            return false;
        }
        self.bytes.extend_from_slice(chunk);
        true
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

pub(crate) fn read_stream_bounded(
    stream: &IInputStream,
    limit: usize,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<u8>> {
    let mut body = BoundedBody::with_limit(limit);
    let buffer = Buffer::Create(READ_CHUNK_BYTES).ok()?;
    let mut chunk = Vec::new();
    loop {
        if cancelled() || Instant::now() >= deadline {
            return None;
        }
        let _ = buffer.SetLength(0);
        let filled = await_until!(
            stream.ReadAsync(&buffer, READ_CHUNK_BYTES, InputStreamOptions::Partial),
            deadline
        )?;
        let length = filled.Length().ok()? as usize;
        if length == 0 {
            return Some(body.into_bytes());
        }
        chunk.resize(length, 0);
        DataReader::FromBuffer(&filled)
            .ok()?
            .ReadBytes(&mut chunk)
            .ok()?;
        if !body.push(&chunk) {
            return None;
        }
    }
}

pub(crate) fn copy_stream_bounded(
    stream: &IInputStream,
    limit: u64,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    mut on_chunk: impl FnMut(&[u8]) -> bool,
) -> Option<u64> {
    let mut received = 0u64;
    let buffer = Buffer::Create(DOWNLOAD_CHUNK_BYTES).ok()?;
    let mut chunk = Vec::new();
    loop {
        if cancelled() || Instant::now() >= deadline {
            return None;
        }
        let _ = buffer.SetLength(0);
        let filled = await_until!(
            stream.ReadAsync(&buffer, DOWNLOAD_CHUNK_BYTES, InputStreamOptions::Partial),
            deadline
        )?;
        let length = filled.Length().ok()? as usize;
        if length == 0 {
            return Some(received);
        }
        let next_total = received.checked_add(length as u64)?;
        if next_total > limit {
            return None;
        }
        chunk.resize(length, 0);
        DataReader::FromBuffer(&filled)
            .ok()?
            .ReadBytes(&mut chunk)
            .ok()?;
        if !on_chunk(&chunk) {
            return None;
        }
        received = next_total;
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedBody;

    #[test]
    fn body_accepts_chunks_up_to_its_limit() {
        let mut body = BoundedBody::with_limit(8);
        assert!(body.push(&[1, 2, 3, 4]));
        assert!(body.push(&[5, 6, 7, 8]));
        assert_eq!(body.remaining(), 0);
        assert_eq!(body.into_bytes(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn body_rejects_the_chunk_that_would_exceed_the_limit() {
        let mut body = BoundedBody::with_limit(6);
        assert!(body.push(&[0; 4]));
        assert!(!body.push(&[0; 3]));
        assert_eq!(body.remaining(), 2);
        assert!(!BoundedBody::with_limit(0).push(&[1]));
        assert!(BoundedBody::with_limit(0).push(&[]));
    }
}
