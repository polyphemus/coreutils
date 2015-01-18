pub mod bytes;

pub trait Select {
    fn select<'a>(&'a mut self, selected: usize) -> Selected<'a>;
    fn consume_line(&mut self) -> Result<usize, String>;
}

pub enum Selected<'a> {
    Complete(&'a [u8]),
    NewlineFound(&'a [u8], usize),
    Partial(&'a [u8], usize),
    Invalid(&'a [u8]),
    EndOfFile,
}
