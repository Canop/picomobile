use core::fmt::Write;

pub struct SliceWriter<'a> {
    buf: &'a mut [u8],
    cursor: usize,
}

impl SliceWriter<'_> {
    pub fn new(buf: &mut [u8]) -> SliceWriter<'_> {
        SliceWriter { buf, cursor: 0 }
    }
    pub fn reset(&mut self) {
        self.cursor = 0;
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.cursor]
    }
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("UTF8_ERROR")
    }
}

impl<'a> Write for SliceWriter<'a> {
    fn write_str(
        &mut self,
        s: &str,
    ) -> core::fmt::Result {
        let bytes = s.as_bytes();
        if self.cursor + bytes.len() > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.cursor..self.cursor + bytes.len()].copy_from_slice(bytes);
        self.cursor += bytes.len();
        Ok(())
    }
}
