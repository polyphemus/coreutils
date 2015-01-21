use std;
use std::io::{IoResult, IoError};

use self::super::{Select, Selected};

pub struct CharsReader<R> {
    reader: R,
    buffer: [u8; 4096],
    start: usize,
    end_valid: usize,  // exclusive
    end: usize,  // exclusive
}

impl<R: Reader> CharsReader<R> {
    pub fn new(reader: R) -> CharsReader<R> {
        let empty_buffer = unsafe {
            std::mem::uninitialized::<[u8; 4096]>()
        };

        CharsReader {
            reader: reader,
            buffer: empty_buffer,
            start: 0,
            end_valid: 0,
            end: 0,
        }
    }

    #[inline]
    fn read(&mut self) -> IoResult<usize> {
        // tables mapping high order bit pattern to the utf-8 values after an AND op.
        let high_order_table = [ 0b1000_0000, 0b1110_0000, 0b1111_0000, 0b1111_1000 ];
        let and_table = [ 0, 0b1100_0000, 0b1110_0000, 0b1111_0000 ];

        let nread = {
            let buffer_fill = self.buffer.slice_from_mut(self.end);

            match self.reader.read(buffer_fill) {
                Ok(nread) => nread,
                error => return error
            }
        };

        self.end += nread;
        self.end_valid = self.end;

        for idx in 0 .. 4 {
            let cur_byte = self.buffer[self.end - idx - 1];

            if cur_byte & high_order_table[idx] == and_table[idx] {
                break;
            }

            if cur_byte & 0b1100_0000 != 0b1000_0000 {
                self.end_valid = self.end - idx - 1;
                break;
            }
        }

        std::str::from_utf8(self.buffer.slice(self.start, self.end_valid)).unwrap();

        Ok(nread)
    }

    #[inline]
    fn maybe_fill_buf(&mut self) -> IoResult<usize> {
        if self.start == self.end_valid {
            // copy the last, at most 4, (invalid utf-8) bytes to the start of the buffer
            // Borrow rules prevent you from copying within one buffer, therefore force it
            let mut_buf: &mut [u8] = unsafe {
                std::mem::transmute(self.buffer.as_slice())
            };
            mut_buf.clone_from_slice(self.buffer.slice(self.end_valid, self.end));

            self.start = 0;
            self.end -= self.end_valid;
            self.end_valid = 0;

            self.read()
        } else {
            Ok(0)
        }
    }
}

impl<R: Reader> Select for CharsReader<R> {
    fn select<'a>(&'a mut self, selected: usize) -> Selected<'a> {
        match self.maybe_fill_buf() {
            Err(IoError { kind: std::io::EndOfFile, .. }) => (),
            Err(err) => panic!("read error: {}", err.desc),
            _ => ()
        }

        if self.start == self.end_valid {
            assert!(self.end_valid == self.end);
            return Selected::EndOfFile;
        }

        let buf = self.buffer.slice(self.start, self.end_valid);
        let string = unsafe { std::str::from_utf8_unchecked(buf) };

        let mut chars = string.char_indices();

        let mut chars_end_idx = self.start;
        for char_idx in 0 .. (selected) {
            match chars.next() {
                Some((_, '\n')) => {
                    chars_end_idx += 1;
                    let segment = self.buffer.slice(self.start, chars_end_idx);
                    self.start = chars_end_idx;

                    return Selected::NewlineFound(segment);
                }
                Some((_, chr)) => {
                    chars_end_idx += chr.len_utf8();
                }
                None => {
                    assert!(chars_end_idx == self.end_valid && char_idx != 0);
                    let segment = self.buffer.slice(self.start, chars_end_idx);
                    self.start = chars_end_idx;

                    return Selected::Partial(segment, char_idx);
                }
            }
        }
        // because the output delimiter should only be placed between
        // segments check if the char after selected is a newline
        if let Some((_, '\n')) = chars.next() {
            let segment = self.buffer.slice(self.start, chars_end_idx + 1);
            self.start = chars_end_idx + 1;

            return Selected::NewlineFound(segment);
        }

        let segment = self.buffer.slice(self.start, chars_end_idx);
        self.start = chars_end_idx;

        Selected::Complete(segment)
    }

    fn consume_line(&mut self) -> Result<usize, String> {
        let mut bytes_consumed = 0;

        loop {
            match self.maybe_fill_buf() {
                Ok(0) | Err(IoError { kind: std::io::EndOfFile, .. }) => {
                    if self.start == self.end {
                        return Ok(bytes_consumed)
                    }
                }
                Err(err) => panic!("read error: {}", err.desc),
                Ok(_) => ()
            }

            let filled_buf = self.buffer.slice(self.start, self.end_valid);

            if let Some(idx) = filled_buf.position_elem(&b'\n') {
                self.start += idx + 1;
                return Ok(bytes_consumed + idx + 1);
            } else {
                bytes_consumed += filled_buf.len();

                self.start = self.end_valid;
            }
        }
    }
}
