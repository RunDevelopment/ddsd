use std::num::NonZeroU32;

use crate::cast;

pub(crate) fn read_u32_le_array(
    reader: &mut impl std::io::Read,
    buffer: &mut [u32],
) -> std::io::Result<()> {
    reader.read_exact(cast::as_bytes_mut(buffer))?;
    for i in buffer.iter_mut() {
        *i = u32::from_le(*i);
    }
    Ok(())
}

/// An implementation of div_ceil to lower MSRV.
pub(crate) fn div_ceil<T>(a: T, b: T) -> T
where
    T: Copy
        + PartialEq
        + PartialOrd
        + From<u8>
        + std::ops::Div<Output = T>
        + std::ops::Rem<Output = T>
        + std::ops::Add<Output = T>
        + Unsigned,
{
    assert!(a >= T::from(0));
    assert!(b > T::from(0));

    let d = a / b;
    if a % b != T::from(0) {
        d + T::from(1)
    } else {
        d
    }
}

pub(crate) fn round_down_to_multiple<T>(value: T, multiple: T) -> T
where
    T: Copy + std::ops::Sub<Output = T> + std::ops::Rem<Output = T> + Unsigned,
{
    value - (value % multiple)
}

pub(crate) trait Unsigned {}
impl Unsigned for u8 {}
impl Unsigned for u16 {}
impl Unsigned for u32 {}
impl Unsigned for u64 {}
impl Unsigned for usize {}

pub(crate) const NON_ZERO_U32_ONE: NonZeroU32 = {
    if let Some(n) = NonZeroU32::new(1) {
        n
    } else {
        panic!()
    }
};

pub(crate) const fn get_mipmap_size(main_size: u32, level: u8) -> NonZeroU32 {
    let size = main_size >> level;
    if let Some(size) = NonZeroU32::new(size) {
        size
    } else {
        NON_ZERO_U32_ONE
    }
}

/// Computes `2^exponent` as a float.
#[inline(always)]
pub(crate) fn two_powi(exponent: i8) -> f32 {
    // Ensure the exponent is within the range for f32
    // Exponent range for f32: -126 to 127 (since 2^127 is the max positive finite power of 2)
    debug_assert!(-126 <= exponent, "Exponent out of range for f32");

    let bits = (((exponent as i32) + 127) as u32) << 23;
    f32::from_bits(bits)
}

/// This is a hack to explicitly annotate the types of the closures.
pub(crate) fn closure_types<A, B, F: Fn(A) -> B>(f: F) -> F {
    f
}
pub(crate) fn closure_types3<A1, A2, A3, B, F: Fn(A1, A2, A3) -> B>(f: F) -> F {
    f
}

/// This can be used to hint to the compiler that a branch is unlikely to be taken.
#[cold]
#[inline]
pub(crate) fn unlikely_branch() {}

/// Skips the exact number of bytes from the reader.
///
/// If it seeks past the end of the file, it will return an EOF error.
pub(crate) fn io_skip_exact<R: std::io::Seek + ?Sized>(
    reader: &mut R,
    count: u64,
) -> std::io::Result<()> {
    if count > i64::MAX as u64 {
        // we will conservatively assume that such large files do not exist
        // and error out with an EOF
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "seeking past end of file",
        ));
    }

    // don't invoke the reader at all if we don't need to skip
    if count == 0 {
        return Ok(());
    }

    let current = reader.stream_position()?;
    // TODO: Use `seek_relative` once the MSRV allows it.
    let actual = reader.seek(std::io::SeekFrom::Current(count as i64))?;

    if actual != current.saturating_add(count) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "seeking past end of file",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn div_ceil() {
        for a in 0..255 {
            for b in 1..255 {
                let expected = (a as f64 / b as f64).ceil() as u8;
                assert_eq!(super::div_ceil(a, b), expected, "a={}, b={}", a, b);
            }
        }
    }
    #[test]
    fn two_powi() {
        for i in -126..=127 {
            let expected = 2.0f32.powi(i as i32);
            let actual = super::two_powi(i);
            assert_eq!(actual, expected, "i={}", i);
        }
    }
}
