use std;
use std::io::{IoResult, IoError};

use self::super::{Select, Selected};

pub struct BytesReader<R> {
    reader: R,
    buffer: [u8; 4096],
    start: usize,
    end: usize,  // exclusive
}

impl<R: Reader> BytesReader<R> {
    pub fn new(reader: R) -> BytesReader<R> {
        let empty_buffer = unsafe {
            std::mem::uninitialized::<[u8; 4096]>()
        };

        BytesReader {
            reader: reader,
            buffer: empty_buffer,
            start: 0,
            end: 0,
        }
    }

    #[inline]
    fn read(&mut self) -> IoResult<usize> {
        let buffer_fill = &mut self.buffer[self.end..];

        match self.reader.read(buffer_fill) {
            Ok(nread) => {
                self.end += nread;
                Ok(nread)
            }
            error => error
        }
    }

    #[inline]
    fn maybe_fill_buf(&mut self) -> IoResult<usize> {
        if self.end == self.start {
            self.start = 0;
            self.end = 0;

            self.read()
        } else {
            Ok(0)
        }
    }
}

impl<R: Reader> Select for BytesReader<R> {
    fn select<'a>(&'a mut self, bytes: usize) -> Selected<'a> {
        match self.maybe_fill_buf() {
            Err(IoError { kind: std::io::EndOfFile, .. }) => (),
            Err(err) => panic!("read error: {}", err.desc),
            _ => ()
        }

        let newline_idx = match self.end - self.start {
            0 => return Selected::EndOfFile,
            buf_used if bytes < buf_used => {
                // because the output delimiter should only be placed between
                // segments check if the byte after bytes is a newline
                let buf_slice = &self.buffer[self.start..self.start + bytes + 1];

                match buf_slice.position_elem(&b'\n') {
                    Some(idx) => idx,
                    None => {
                        let segment = &self.buffer[self.start..self.start + bytes];

                        self.start += bytes;

                        return Selected::Complete(segment);
                    }
                }
            }
            _ => {
                let buf_filled = &self.buffer[self.start..self.end];

                match buf_filled.position_elem(&b'\n') {
                    Some(idx) => idx,
                    None => {
                        let segment = &self.buffer[self.start..self.end];

                        self.start = 0;
                        self.end = 0;

                        return Selected::Partial(segment, segment.len());
                    }
                }
            }
        };

        let new_start = self.start + newline_idx + 1;
        let segment = &self.buffer[self.start..new_start];

        self.start = new_start;
        Selected::NewlineFound(segment)
    }

    fn consume_line(&mut self) -> Result<usize, String> {
        let mut bytes_consumed = 0;

        loop {
            match self.maybe_fill_buf() {
                Ok(0) | Err(IoError { kind: std::io::EndOfFile, .. })
                    if self.start == self.end => return Ok(bytes_consumed),
                Err(err) => panic!("read error: {}", err.desc),
                _ => ()
            }

            let filled_buf = &self.buffer[self.start..self.end];

            match filled_buf.position_elem(&b'\n') {
                Some(idx) => {
                    self.start += idx + 1;
                    return Ok(bytes_consumed + idx + 1);
                }
                _ => ()
            }

            bytes_consumed += filled_buf.len();

            self.start = 0;
            self.end = 0;
        }
    }
}
