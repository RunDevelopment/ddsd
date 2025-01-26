//! An internal module with helper methods for reading bytes from a reader, and
//! writing decoded pixels to the output buffer.

use std::io::{Read, SeekFrom};
use std::mem::size_of;

use crate::{cast, util::div_ceil, DecodeError, Rect, Size};

use super::ReadSeek;

/// A function that processes a row of pixels.
///
/// The first argument is a byte slice of encoded pixels. The slice is
/// guaranteed te have a length that is a multiple of `size_of::<InPixel>()`.
///
/// The second argument is a byte slice of decoded pixels. The slice is
/// guaranteed te have a length that is a multiple of `size_of::<OutputPixel>()`.
///
/// Both slices are guaranteed to have the same number of pixels.
pub(crate) type ProcessPixelsFn = fn(encoded: &[u8], decoded: &mut [u8]);

/// A helper function for implementing [`ProcessPixelsFn`]s.
#[inline]
pub(crate) fn process_pixels_helper<InPixel: cast::FromLeBytes, OutPixel: cast::IntoNeBytes>(
    encoded: &[u8],
    decoded: &mut [u8],
    f: impl Fn(InPixel) -> OutPixel,
) {
    // group bytes into chunks
    let encoded: &[InPixel::Bytes] = cast::from_bytes(encoded).expect("Invalid input buffer");
    let decoded: &mut [OutPixel::Bytes] =
        cast::from_bytes_mut(decoded).expect("Invalid output buffer");

    for (encoded, decoded) in encoded.iter().zip(decoded.iter_mut()) {
        let input: InPixel = cast::FromLeBytes::from_le_bytes(*encoded);
        *decoded = cast::IntoNeBytes::into_ne_bytes(f(input));
    }
}

/// Helper method for decoding UNCOMPRESSED formats.
pub(crate) fn for_each_pixel_untyped<InPixel, OutPixel>(
    r: &mut dyn Read,
    buf: &mut [u8],
    process_pixels: ProcessPixelsFn,
) -> Result<(), DecodeError> {
    fn inner(
        r: &mut dyn Read,
        buf: &mut [u8],
        size_of_in: usize,
        size_of_out: usize,
        process_pixels: ProcessPixelsFn,
    ) -> Result<(), DecodeError> {
        assert!(buf.len() % size_of_out == 0);
        let pixels = buf.len() / size_of_out;

        let mut read_buffer = UntypedPixelBuffer::new(pixels, size_of_in);
        for buf in buf.chunks_mut(read_buffer.buffered_pixels() * size_of_out) {
            let row = read_buffer.read(r)?;
            debug_assert!(row.len() % size_of_in == 0);
            debug_assert!(buf.len() % size_of_out == 0);
            debug_assert_eq!(row.len() / size_of_in, buf.len() / size_of_out);
            process_pixels(row, buf);
        }
        Ok(())
    }

    inner(
        r,
        buf,
        size_of::<InPixel>(),
        size_of::<OutPixel>(),
        process_pixels,
    )
}

/// Helper method for decoding UNCOMPRESSED formats.
///
/// `process_pixels` has the same semantics as in `for_each_pixel_untyped`.
pub(crate) fn for_each_pixel_rect_untyped<InPixel, OutPixel>(
    r: &mut dyn ReadSeek,
    buf: &mut [u8],
    row_pitch: usize,
    size: Size,
    rect: Rect,
    process_pixels: ProcessPixelsFn,
) -> Result<(), DecodeError> {
    #[allow(clippy::too_many_arguments)]
    fn inner(
        r: &mut dyn ReadSeek,
        buf: &mut [u8],
        row_pitch: usize,
        size: Size,
        rect: Rect,
        size_of_in: usize,
        size_of_out: usize,
        process_pixels: ProcessPixelsFn,
    ) -> Result<(), DecodeError> {
        // assert that no overflow will occur for byte positions in the encoded image/reader
        assert!(size
            .pixels()
            .checked_mul(size_of_in as u64)
            .map(|bytes| bytes <= i64::MAX as u64)
            .unwrap_or(false));

        let encoded_bytes_per_row = size.width as i64 * size_of_in as i64;
        let encoded_bytes_before_rect = rect.x as i64 * size_of_in as i64;
        let encoded_bytes_after_rect =
            (size.width - rect.x - rect.width) as i64 * size_of_in as i64;

        // jump to the first pixel
        seek_relative(
            r,
            encoded_bytes_per_row * rect.y as i64 + encoded_bytes_before_rect,
        )?;

        let pixels_per_line = rect.width as usize;
        let mut row: Box<[u8]> =
            vec![Default::default(); pixels_per_line * size_of_in].into_boxed_slice();
        for y in 0..rect.height {
            if y > 0 {
                // jump to the first pixel in the next row
                // (this has already been done for the first row; see above)
                seek_relative(r, encoded_bytes_before_rect + encoded_bytes_after_rect)?;
            }

            // read next line
            r.read_exact(&mut row)?;

            let buf_start = y as usize * row_pitch;
            let buf_len = pixels_per_line * size_of_out;
            let buf = &mut buf[buf_start..(buf_start + buf_len)];
            debug_assert_eq!(row.len() / size_of_in, buf.len() / size_of_out);
            process_pixels(&row, buf);
        }

        // jump to the end of the surface to put the reader into a known position
        seek_relative(
            r,
            encoded_bytes_after_rect
                + (size.height - rect.y - rect.height) as i64 * encoded_bytes_per_row,
        )?;

        Ok(())
    }

    inner(
        r,
        buf,
        row_pitch,
        size,
        rect,
        size_of::<InPixel>(),
        size_of::<OutPixel>(),
        process_pixels,
    )
}

fn seek_relative(r: &mut dyn ReadSeek, offset: i64) -> std::io::Result<()> {
    if offset != 0 {
        r.seek(SeekFrom::Current(offset))?;
    }
    Ok(())
}

/// A function that processes a row of blocks.
///
/// Arguments:
///
/// `encoded_blocks` is a byte slice of blocks. The slice is
/// guaranteed te have a length that is a multiple of `BYTES_PER_BLOCK`.
///
/// `decoded` is a byte slice of decoded pixels.
///
/// `width` is the number of pixels in a row. This might *not* be a multiple
/// of `BLOCK_SIZE_X`
///
/// `stride` is the number of bytes between the start of two consecutive rows
/// in `decoded`.
///
/// `rows` is the number of rows to decode. This is at least 1 and at most
/// `BLOCK_SIZE_Y`.
pub(crate) type ProcessBlocksFn =
    fn(encoded_blocks: &[u8], decoded: &mut [u8], width: usize, stride: usize, rows: usize);

/// A helper function for implementing [`ProcessBlocksFn`]s.
#[inline]
pub(crate) fn process_2x1_blocks_helper<
    const BYTES_PER_BLOCK: usize,
    OutPixel: cast::IntoNeBytes,
>(
    encoded_blocks: &[u8],
    decoded: &mut [u8],
    width: usize,
    process_block: impl Fn([u8; BYTES_PER_BLOCK]) -> [OutPixel; 2],
) {
    // group bytes into chunks
    let encoded_blocks: &[[u8; BYTES_PER_BLOCK]] =
        cast::from_bytes(encoded_blocks).expect("Invalid block buffer");

    let decoded = &mut decoded[..width * size_of::<OutPixel>()];
    let decoded: &mut [OutPixel::Bytes] =
        cast::from_bytes_mut(decoded).expect("Invalid output buffer");
    debug_assert!(decoded.len() == width);

    let width_half = width / 2;

    // do full pairs first
    let decoded_pairs: &mut [[OutPixel::Bytes; 2]] =
        cast::as_array_chunks_mut(&mut decoded[..(width_half * 2)]).unwrap();
    for (encoded, decoded) in encoded_blocks.iter().zip(decoded_pairs.iter_mut()) {
        let [p0, p1] = process_block(*encoded);
        decoded[0] = cast::IntoNeBytes::into_ne_bytes(p0);
        decoded[1] = cast::IntoNeBytes::into_ne_bytes(p1);
    }

    // last lone pixel (if any)
    if width % 2 == 1 {
        let encoded = encoded_blocks.last().unwrap();
        let [p0, _] = process_block(*encoded);
        decoded[width - 1] = cast::IntoNeBytes::into_ne_bytes(p0);
    }
}
/// A helper function for implementing [`ProcessBlocksFn`]s.
#[inline]
pub(crate) fn process_4x4_blocks_helper<
    const BYTES_PER_BLOCK: usize,
    OutPixel: cast::IntoNeBytes + cast::Castable + Copy,
>(
    encoded_blocks: &[u8],
    decoded: &mut [u8],
    width: usize,
    stride: usize,
    row_count: usize,
    process_block: impl Fn([u8; BYTES_PER_BLOCK]) -> [OutPixel; 16],
) {
    debug_assert!(row_count <= 4);

    // group bytes into chunks
    let encoded_blocks: &[[u8; BYTES_PER_BLOCK]] =
        cast::from_bytes(encoded_blocks).expect("Invalid block buffer");

    if width % 4 == 0 && row_count == 4 && stride % size_of::<OutPixel>() == 0 {
        if let Some(decoded) = cast::from_bytes_mut::<OutPixel>(decoded) {
            let stride = stride / size_of::<OutPixel>();
            for (block_index, block) in encoded_blocks.iter().enumerate() {
                let pixel_index = block_index * 4;

                let block = process_block(*block);

                for y in 0..4 {
                    let row_start = stride * y + pixel_index;
                    let row = &mut decoded[row_start..row_start + 4];
                    for x in 0..4 {
                        row[x] = block[y * 4 + x];
                    }
                }
            }

            return;
        }
    }

    // General implementation. Slower.
    let block_h = row_count;
    for (block_index, block) in encoded_blocks.iter().enumerate() {
        let pixel_index = block_index * 4;
        let block_w = 4.min(width - pixel_index);

        let block = process_block(*block);
        for y in 0..block_h {
            let row_start = stride * y + pixel_index * size_of::<OutPixel>();
            let row = &mut decoded[row_start..];
            let row: &mut [OutPixel::Bytes] =
                cast::from_bytes_mut(row).expect("Invalid output buffer");
            for x in 0..block_w {
                row[x] = cast::IntoNeBytes::into_ne_bytes(block[y * 4 + x]);
            }
        }
    }
}
/// A helper function for implementing [`ProcessBlocksFn`]s.
///
/// This is a *general* implementation. It will work with any block size, but
/// it's a lot slower than the specialized versions. Don't use this directly.
/// Instead, use it as the starting point for a specialized implementation.
pub(crate) fn _process_blocks_helper<
    const BLOCK_SIZE_X: usize,
    const BLOCK_SIZE_Y: usize,
    const BLOCK_PIXELS: usize,
    const BYTES_PER_BLOCK: usize,
    OutPixel: cast::IntoNeBytes + Copy,
>(
    encoded_blocks: &[u8],
    decoded: &mut [u8],
    width: usize,
    stride: usize,
    row_count: usize,
    process_block: impl Fn([u8; BYTES_PER_BLOCK]) -> [OutPixel; BLOCK_PIXELS],
) {
    debug_assert_eq!(BLOCK_SIZE_X * BLOCK_SIZE_Y, BLOCK_PIXELS);

    // group bytes into chunks
    let encoded_blocks: &[[u8; BYTES_PER_BLOCK]] =
        cast::from_bytes(encoded_blocks).expect("Invalid block buffer");

    let block_h = row_count;
    for (block_index, block) in encoded_blocks.iter().enumerate() {
        let pixel_index = block_index * BLOCK_SIZE_X;
        let block_w = BLOCK_SIZE_X.min(width - pixel_index);

        let block = process_block(*block);
        for y in 0..block_h {
            let row_start = y * stride + pixel_index * size_of::<OutPixel>();
            let row = &mut decoded[row_start..];
            let row: &mut [OutPixel::Bytes] =
                cast::from_bytes_mut(row).expect("Invalid output buffer");
            for x in 0..block_w {
                row[x] = cast::IntoNeBytes::into_ne_bytes(block[y * BLOCK_SIZE_X + x]);
            }
        }
    }
}

pub(crate) fn for_each_block_untyped<
    const BLOCK_SIZE_X: usize,
    const BLOCK_SIZE_Y: usize,
    const BYTES_PER_BLOCK: usize,
    OutPixel,
>(
    r: &mut dyn Read,
    buf: &mut [u8],
    size: Size,
    process_pixels: ProcessBlocksFn,
) -> Result<(), DecodeError> {
    fn inner(
        r: &mut dyn Read,
        buf: &mut [u8],
        size: Size,
        block_size: (usize, usize),
        bytes_per_block: usize,
        size_of_out: usize,
        process_pixels: ProcessBlocksFn,
    ) -> Result<(), DecodeError> {
        // The basic idea here is to decode the image line by line. A line is a
        // sequence of encoded blocks that together describe BLOCK_SIZE_Y rows of
        // pixels in the final image.
        //
        // Since reading a bunch of small lines from disk is slow, we allocate one
        // large buffer to hold N lines at a time. The we process the lines in the
        // buffer and refill as needed.

        assert!(!size.is_empty());

        let (block_size_x, block_size_y) = block_size;
        let width_blocks = div_ceil(size.width, block_size_x as u32) as usize;
        let height_blocks = div_ceil(size.height, block_size_y as u32) as usize;

        let mut line_buffer = UntypedLineBuffer::new(width_blocks * bytes_per_block, height_blocks);

        let mut block_y = 0;
        while let Some(block_line) = line_buffer.next_line(r)? {
            // how many rows of pixels we'll decode
            // this is usually BLOCK_SIZE_Y, but might be less for the last block
            let pixel_rows = block_size_y.min(size.height as usize - block_y * block_size_y);
            let pixel_row_bytes = size.width as usize * size_of_out;
            debug_assert!(buf.len() % pixel_row_bytes == 0);

            let start_pixel_y = block_y * block_size_y;
            let buf = &mut buf
                [start_pixel_y * pixel_row_bytes..(start_pixel_y + pixel_rows) * pixel_row_bytes];

            process_pixels(
                block_line,
                buf,
                size.width as usize,
                pixel_row_bytes,
                pixel_rows,
            );

            block_y += 1;
        }
        Ok(())
    }

    inner(
        r,
        buf,
        size,
        (BLOCK_SIZE_X, BLOCK_SIZE_Y),
        BYTES_PER_BLOCK,
        size_of::<OutPixel>(),
        process_pixels,
    )
}

/// A buffer holding raw encoded pixels straight from the reader.
struct UntypedPixelBuffer {
    buf: Vec<u8>,
    bytes_per_pixel: usize,
    bytes_left: usize,
}
impl UntypedPixelBuffer {
    /// The target buffer size is in bytes. Currently 64 KiB.
    const TARGET: usize = 64 * 1024;

    fn new(pixels: usize, bytes_per_pixel: usize) -> Self {
        let buf_bytes = (Self::TARGET / bytes_per_pixel).min(pixels) * bytes_per_pixel;
        let buf = vec![0; buf_bytes];
        Self {
            buf,
            bytes_per_pixel,
            bytes_left: pixels * bytes_per_pixel,
        }
    }

    fn buffered_pixels(&self) -> usize {
        self.buf.len() / self.bytes_per_pixel
    }

    fn read<R: Read + ?Sized>(&mut self, r: &mut R) -> Result<&[u8], DecodeError> {
        let bytes_to_read = self.buf.len().min(self.bytes_left);
        assert!(bytes_to_read > 0);
        self.bytes_left -= bytes_to_read;

        let buf = &mut self.buf[..bytes_to_read];
        r.read_exact(buf)?;
        Ok(buf)
    }
}

/// A buffer holding raw encoded lines of pixels straight from the reader.
struct UntypedLineBuffer {
    buf: Vec<u8>,
    bytes_per_line: usize,
    /// How many lines are still left to read from disk
    lines_on_disk: usize,
    /// The index at which the current line starts in the buffer.
    ///
    /// If `>= buffer.len()`, the buffer is empty and needs to be refilled.
    current_line_start: usize,
}
impl UntypedLineBuffer {
    fn new(bytes_per_line: usize, height: usize) -> Self {
        const TARGET_BUFFER_SIZE: usize = 64 * 1024; // 64 KB

        let lines_in_buffer = (TARGET_BUFFER_SIZE / bytes_per_line).clamp(1, height);
        let buf_len = lines_in_buffer * bytes_per_line;
        // TODO: protect against allocating very large buffers (> 1 MB)
        let buf = vec![0_u8; buf_len];

        Self {
            buf,
            bytes_per_line,
            lines_on_disk: height,
            current_line_start: buf_len,
        }
    }
    fn next_line(&mut self, r: &mut dyn Read) -> Result<Option<&[u8]>, DecodeError> {
        if self.current_line_start >= self.buf.len() {
            if self.lines_on_disk == 0 {
                // all lines have been read
                return Ok(None);
            }

            // refill the buffer
            let lines_to_read = (self.buf.len() / self.bytes_per_line).min(self.lines_on_disk);
            self.lines_on_disk -= lines_to_read;
            self.buf.truncate(lines_to_read * self.bytes_per_line);
            r.read_exact(cast::as_bytes_mut(&mut self.buf))?;
            self.current_line_start = 0;
        }

        // get a line from the buffer
        let line_end = self.current_line_start + self.bytes_per_line;
        let line = &self.buf[self.current_line_start..line_end];
        self.current_line_start = line_end;
        Ok(Some(line))
    }
}
