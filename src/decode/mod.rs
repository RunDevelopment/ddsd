use crate::Precision;

mod adapt;
mod bc;
mod bc7;
mod convert;
mod decoder;
mod read_write;
mod sub_sampled;
mod uncompressed;

pub(crate) use bc::*;
pub(crate) use decoder::*;
pub(crate) use sub_sampled::*;
pub(crate) use uncompressed::*;

pub(crate) trait WithPrecision {
    const PRECISION: Precision;
}
impl WithPrecision for u8 {
    const PRECISION: Precision = Precision::U8;
}
impl WithPrecision for u16 {
    const PRECISION: Precision = Precision::U16;
}
impl WithPrecision for f32 {
    const PRECISION: Precision = Precision::F32;
}
