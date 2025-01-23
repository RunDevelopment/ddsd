use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ddsd::*;
use rand::{Rng, RngCore};

fn simple_texture_header(size: Size, format: DxgiFormat) -> Header {
    Header {
        flags: DdsFlags::REQUIRED,
        height: size.height,
        width: size.width,
        depth: None,
        mipmap_count: None,
        pixel_format: PixelFormat {
            flags: PixelFormatFlags::FOURCC,
            four_cc: Some(FourCC::DX10),
            rgb_bit_count: 0,
            r_bit_mask: 0,
            g_bit_mask: 0,
            b_bit_mask: 0,
            a_bit_mask: 0,
        },
        caps: DdsCaps::REQUIRED,
        caps2: DdsCaps2::empty(),
        dxt10: Some(HeaderDxt10 {
            dxgi_format: format,
            resource_dimension: ResourceDimension::Texture2D,
            misc_flag: MiscFlags::empty(),
            array_size: 1,
            misc_flags2: MiscFlags2::empty(),
        }),
    }
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = vec![0; len];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut out);
    out
}

fn bench_decoder(c: &mut Criterion, format: DxgiFormat, channels: Channels, precision: Precision) {
    bench_decoder_with_data(c, format, channels, precision, |_| {});
}
fn bench_decoder_with_data(
    c: &mut Criterion,
    format: DxgiFormat,
    channels: Channels,
    precision: Precision,
    mut data_modifier: impl FnMut(&mut [u8]),
) {
    let color = ColorFormat::new(channels, precision);
    let name = format!("{:?} -> {}", format, color);

    c.bench_function(&name, |b| {
        let header = simple_texture_header((4096, 4096).into(), format);

        let reader = DdsDecoder::from_header(header).unwrap();
        let format = reader.format();
        assert!(format.supports(color));

        let surface = reader.layout().texture().unwrap().main();
        let mut bytes = random_bytes(surface.data_len() as usize).into_boxed_slice();
        data_modifier(&mut bytes);
        let mut output =
            vec![0; surface.size().pixels() as usize * color.bytes_per_pixel() as usize];
        b.iter(|| {
            let result = format.decode(
                black_box(&mut bytes.as_ref()),
                surface.size(),
                color,
                black_box(&mut output),
            );
            black_box(result).unwrap();
        });
    });
}

/// This sets the BC7 block modes such that each mode is equally likely.
///
/// This is necessary, because the block mode is decided by the number of
/// leading zeros, meaning that for random bytes, 50% of the blocks will be
/// mode 0. This does NOT represent real-world data at all, hence this function.
///
/// Note that this is not a perfect solution either, but it should be good
/// enough.
fn random_bc7_modes(data: &mut [u8]) {
    let mut rng = rand::thread_rng();
    for i in (0..data.len()).step_by(16) {
        let mode: u8 = rng.gen_range(0..8);
        let mut byte = data[i];
        byte |= 1;
        byte <<= mode;
        data[i] = byte;
    }
}

pub fn uncompressed(c: &mut Criterion) {
    use Channels::*;
    use Precision::*;

    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, U16);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, F32);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgb, U8);
    bench_decoder(c, DxgiFormat::R16G16_SNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::B8G8R8X8_UNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::BC1_UNORM, Rgba, U8);
    bench_decoder_with_data(c, DxgiFormat::BC7_UNORM, Rgba, U8, random_bc7_modes);
}

criterion_group!(benches, uncompressed);
criterion_main!(benches);
