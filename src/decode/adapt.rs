use crate::{cast, Channels, Precision};

use super::{convert::Norm, read_write::ProcessPixelsFn};

pub(crate) fn adapt(mut adapter: impl Adapter, from: Channels, to: Channels, precision: Precision) {
    if from == to {
        adapter.direct();
        return;
    }

    match precision {
        Precision::U8 => adapt_for::<u8, _>(adapter, from, to),
        Precision::U16 => adapt_for::<u16, _>(adapter, from, to),
        Precision::F32 => adapt_for::<f32, _>(adapter, from, to),
    }
}
pub(crate) fn adapt_for<Precision, A>(mut adapter: A, from: Channels, to: Channels)
where
    Precision: Norm + cast::Castable + cast::IntoNeBytes,
    [Precision; 1]: cast::IntoNeBytes,
    [Precision; 3]: cast::IntoNeBytes,
    [Precision; 4]: cast::IntoNeBytes,
    A: Adapter,
{
    match (from, to) {
        // no conversion needed
        (Channels::Grayscale, Channels::Grayscale)
        | (Channels::Alpha, Channels::Alpha)
        | (Channels::Rgb, Channels::Rgb)
        | (Channels::Rgba, Channels::Rgba) => adapter.direct(),

        // format without alpha to alpha
        (Channels::Grayscale, Channels::Alpha) | (Channels::Rgb, Channels::Alpha) => {
            adapter.fill(Precision::ONE);
        }
        // alpha to format without alpha
        (Channels::Alpha, Channels::Grayscale) | (Channels::Alpha, Channels::Rgb) => {
            adapter.fill(Precision::ZERO);
        }

        (Channels::Grayscale, Channels::Rgb) => adapter.map(grayscale_to_rgb::<Precision>),
        (Channels::Grayscale, Channels::Rgba) => adapter.map(grayscale_to_rgba::<Precision>),
        (Channels::Alpha, Channels::Rgba) => adapter.map(alpha_to_rgba::<Precision>),
        (Channels::Rgb, Channels::Grayscale) => adapter.map(rgb_to_grayscale::<Precision>),
        (Channels::Rgb, Channels::Rgba) => adapter.map(rgb_to_rgba::<Precision>),
        (Channels::Rgba, Channels::Grayscale) => adapter.map(rgba_to_grayscale::<Precision>),
        (Channels::Rgba, Channels::Alpha) => adapter.map(rgba_to_alpha::<Precision>),
        (Channels::Rgba, Channels::Rgb) => adapter.map(rgba_to_rgb::<Precision>),
    }
}

pub(crate) trait Adapter {
    fn direct(&mut self);

    fn fill<T: cast::IntoNeBytes>(&mut self, value: T);

    fn map<From, To>(&mut self, f: impl Fn(From) -> To)
    where
        From: cast::Castable + Default + Copy,
        To: cast::Castable + cast::IntoNeBytes;
}

pub(crate) struct UncompressedAdapter<'a, 'b> {
    pub encoded: &'a [u8],
    pub decoded: &'b mut [u8],
    pub process_fn: ProcessPixelsFn,
}
impl Adapter for UncompressedAdapter<'_, '_> {
    fn direct(&mut self) {
        (self.process_fn)(self.encoded, self.decoded)
    }

    fn fill<T: cast::IntoNeBytes>(&mut self, value: T) {
        let decoded: &mut [T::Bytes] =
            cast::from_bytes_mut(self.decoded).expect("invalid decoded buffer");

        decoded.fill(value.into_ne_bytes());
    }

    fn map<From, To>(&mut self, f: impl Fn(From) -> To)
    where
        From: cast::Castable + Default + Copy,
        To: cast::Castable + cast::IntoNeBytes,
    {
        // Create a small buffer on the stack. Since the largest pixel size is
        // 16 bytes (RGBA F32), this buffer will be at most 1024 bytes.
        //
        // From my testing, a larger buffer does not improve performance.
        const BUFFER_PIXELS: usize = 64;
        let mut buffer: [From; BUFFER_PIXELS] = [Default::default(); BUFFER_PIXELS];

        let decoded: &mut [To::Bytes] =
            cast::from_bytes_mut(self.decoded).expect("invalid decoded buffer");

        let pixels = decoded.len();

        // bytes per pixel
        debug_assert!(self.encoded.len() % pixels == 0);
        let encoded_bpp = self.encoded.len() / pixels;

        // convert the pixels in small chunks
        for (encoded, decoded) in self
            .encoded
            .chunks(BUFFER_PIXELS * encoded_bpp)
            .zip(decoded.chunks_mut(BUFFER_PIXELS))
        {
            debug_assert!(encoded.len() / encoded_bpp == decoded.len());

            // decode pixels into buffer
            (self.process_fn)(encoded, cast::as_bytes_mut(&mut buffer[..decoded.len()]));

            // convert pixels
            for (decoded, pixel) in decoded.iter_mut().zip(&buffer) {
                *decoded = To::into_ne_bytes(f(*pixel));
            }
        }
    }
}

fn alpha_to_rgba<Precision: Norm>(pixel: [Precision; 1]) -> [Precision; 4] {
    [Norm::ZERO, Norm::ZERO, Norm::ZERO, pixel[0]]
}
fn grayscale_to_rgb<Precision: Norm>(pixel: [Precision; 1]) -> [Precision; 3] {
    [pixel[0], pixel[0], pixel[0]]
}
fn grayscale_to_rgba<Precision: Norm>(pixel: [Precision; 1]) -> [Precision; 4] {
    [pixel[0], pixel[0], pixel[0], Norm::ONE]
}
fn rgb_to_grayscale<Precision: Norm>(pixel: [Precision; 3]) -> [Precision; 1] {
    [pixel[0]]
}
fn rgb_to_rgba<Precision: Norm>(pixel: [Precision; 3]) -> [Precision; 4] {
    [pixel[0], pixel[1], pixel[2], Norm::ONE]
}
fn rgba_to_grayscale<Precision: Norm>(pixel: [Precision; 4]) -> [Precision; 1] {
    [pixel[0]]
}
fn rgba_to_alpha<Precision: Norm>(pixel: [Precision; 4]) -> [Precision; 1] {
    [pixel[3]]
}
fn rgba_to_rgb<Precision: Norm>(pixel: [Precision; 4]) -> [Precision; 3] {
    [pixel[0], pixel[1], pixel[2]]
}
