use std::io::Read;

use crate::{cast, DecodeError};

#[inline(always)]
pub(crate) fn for_each_pixel<const N: usize, const M: usize, InChannel, OutChannel>(
    r: &mut dyn Read,
    buf: &mut [u8],
    process_pixel: impl Fn([InChannel; N]) -> [OutChannel; M],
) -> Result<(), DecodeError>
where
    InChannel: FromLe,
    [InChannel::Raw; N]: cast::Castable + Default,
    [OutChannel; M]: cast::Castable + Default,
{
    let out_pixel_size = size_of::<[OutChannel; M]>();
    assert!(buf.len() % out_pixel_size == 0);
    let pixels = buf.len() / out_pixel_size;

    let mut read_buffer: ReadBuffer<[InChannel::Raw; N]> = ReadBuffer::new(pixels);
    let mut write_aligned: AlignedBuffer<[OutChannel; M]> = AlignedBuffer::new();
    for buf in buf.chunks_mut(read_buffer.buf_pixels * write_aligned.element_size()) {
        let row = read_buffer.read(r)?;

        write_aligned.write(buf, |buf| {
            for (pixel, out) in row.iter().zip(buf) {
                *out = process_pixel(pixel.map(FromLe::from_le));
            }
        });
    }
    Ok(())
}

pub(crate) trait FromLe {
    type Raw;

    fn from_le(raw: Self::Raw) -> Self;
}
impl FromLe for u8 {
    type Raw = u8;

    fn from_le(raw: Self::Raw) -> Self {
        raw
    }
}
impl FromLe for u16 {
    type Raw = u16;

    fn from_le(raw: Self::Raw) -> Self {
        u16::from_le(raw)
    }
}
impl FromLe for u32 {
    type Raw = u32;

    fn from_le(raw: Self::Raw) -> Self {
        u32::from_le(raw)
    }
}
impl FromLe for f32 {
    type Raw = u32;

    fn from_le(raw: Self::Raw) -> Self {
        f32::from_bits(u32::from_le(raw))
    }
}

struct ReadBuffer<T> {
    buf: Vec<T>,
    buf_pixels: usize,
    pixels_left: usize,
}
impl<T> ReadBuffer<T> {
    /// The target buffer size is in bytes. Currently 64 KiB.
    const TARGET: usize = 64 * 1024;

    fn new(pixels: usize) -> Self
    where
        T: Default + Clone,
    {
        let bytes_per_pixel = size_of::<T>();
        let buf_pixels = (Self::TARGET / bytes_per_pixel).min(pixels);
        let buf = vec![T::default(); buf_pixels];
        Self {
            buf,
            buf_pixels,
            pixels_left: pixels,
        }
    }

    fn read(&mut self, r: &mut dyn Read) -> Result<&[T], DecodeError>
    where
        T: cast::Castable,
    {
        let pixels_to_read = self.buf_pixels.min(self.pixels_left);
        self.pixels_left -= pixels_to_read;

        let buf = &mut self.buf[..pixels_to_read];
        r.read_exact(cast::as_bytes_mut(buf))?;
        Ok(buf)
    }
}

struct AlignedBuffer<T> {
    temp_buf: Option<Vec<T>>,
}
impl<T> AlignedBuffer<T> {
    fn new() -> Self {
        Self { temp_buf: None }
    }

    fn element_size(&self) -> usize {
        size_of::<T>()
    }

    fn write(&mut self, buf: &mut [u8], write: impl FnOnce(&mut [T]))
    where
        T: cast::Castable + Default + Copy,
    {
        if let Ok(buf) = bytemuck::try_cast_slice_mut(buf) {
            // the buffer is aligned already
            write(buf);
        } else {
            // use a temporary buffer and copy over
            let size = size_of::<T>();
            assert_eq!(buf.len() % size, 0);
            let len = buf.len() / size;

            let temp = if let Some(temp) = self.temp_buf.as_mut() {
                assert!(temp.len() >= len);
                &mut temp[..len]
            } else {
                let temp = vec![T::default(); len];
                self.temp_buf = Some(temp);
                // we just assigned a value, so unwrap is okay
                self.temp_buf.as_mut().unwrap()
            };

            write(temp);

            buf.copy_from_slice(cast::as_bytes(temp));
        }
    }
}

pub(crate) fn le_to_native_endian_16(buf: &mut [u8]) {
    assert!(buf.len() % 2 == 0);

    if cfg!(target_endian = "big") {
        // TODO: optimize this for when the buffer is aligned to u16/u32/u64
        for i in (0..buf.len()).step_by(2) {
            buf.swap(i, i + 1);
        }
    }
}
pub(crate) fn le_to_native_endian_32(buf: &mut [u8]) {
    assert!(buf.len() % 4 == 0);

    if cfg!(target_endian = "big") {
        // TODO: optimize this for when the buffer is aligned to u32/u64
        for i in (0..buf.len()).step_by(4) {
            buf.swap(i, i + 3);
            buf.swap(i + 1, i + 2);
        }
    }
}
