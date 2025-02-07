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

struct BenchConfig {
    data_modifier: fn(&mut [u8]),
    size: Size,
    name: &'static str,
}
impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            data_modifier: |_| {},
            size: (4096, 4096).into(),
            name: "",
        }
    }
}
fn bench_decoder(c: &mut Criterion, format: DxgiFormat, channels: Channels, precision: Precision) {
    bench_decoder_with(c, format, channels, precision, |_| {});
}
fn bench_decoder_with(
    c: &mut Criterion,
    format: DxgiFormat,
    channels: Channels,
    precision: Precision,
    create_config: impl FnOnce(&mut BenchConfig),
) {
    let mut config = BenchConfig::default();
    create_config(&mut config);

    let color = ColorFormat::new(channels, precision);
    let mut name = format!("{:?} -> {}", format, color);
    if !config.name.is_empty() {
        name += " - ";
        name += config.name;
    }

    c.bench_function(&name, |b| {
        let header = simple_texture_header(config.size, format);

        let reader = DdsDecoder::from_header(header).unwrap();
        let format = reader.format();
        assert!(format.supports(color));

        let surface = reader.layout().texture().unwrap().main();
        let mut bytes = random_bytes(surface.data_len() as usize).into_boxed_slice();
        (config.data_modifier)(&mut bytes);
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
fn set_bc7_modes(data: &mut [u8], mode: u8) {
    for i in (0..data.len()).step_by(16) {
        let mut byte = data[i];
        byte |= 1;
        byte <<= mode;
        data[i] = byte;
    }
}

pub fn uncompressed(c: &mut Criterion) {
    use Channels::*;
    use Precision::*;

    // uncompressed formats
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, U16);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgba, F32);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgb, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgb, U16);
    bench_decoder(c, DxgiFormat::R8G8B8A8_UNORM, Rgb, F32);

    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgba, U16);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgba, F32);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgb, U8);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgb, U16);
    bench_decoder(c, DxgiFormat::R8G8B8A8_SNORM, Rgb, F32);

    bench_decoder(c, DxgiFormat::R16G16_SNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::B8G8R8X8_UNORM, Rgba, U8);
    bench_decoder(c, DxgiFormat::R9G9B9E5_SHAREDEXP, Rgb, U8);

    bench_decoder(c, DxgiFormat::R16G16B16A16_FLOAT, Rgba, U8);
    bench_decoder(c, DxgiFormat::R16G16B16A16_FLOAT, Rgba, U16);
    bench_decoder(c, DxgiFormat::R16G16B16A16_FLOAT, Rgba, F32);

    bench_decoder(c, DxgiFormat::R32G32B32A32_FLOAT, Rgba, U8);
    bench_decoder(c, DxgiFormat::R32G32B32A32_FLOAT, Rgba, U16);
    bench_decoder(c, DxgiFormat::R32G32B32A32_FLOAT, Rgba, F32);

    bench_decoder(c, DxgiFormat::R11G11B10_FLOAT, Rgba, U8);
    bench_decoder(c, DxgiFormat::R11G11B10_FLOAT, Rgba, U16);
    bench_decoder(c, DxgiFormat::R11G11B10_FLOAT, Rgba, F32);

    // sub-sampled formats
    bench_decoder(c, DxgiFormat::R8G8_B8G8_UNORM, Rgb, U8);

    // block-compressed formats
    bench_decoder(c, DxgiFormat::BC1_UNORM, Rgba, U8);
    bench_decoder_with(c, DxgiFormat::BC1_UNORM, Rgba, U8, |c| {
        c.size = (4095, 4095).into();
    });
    bench_decoder(c, DxgiFormat::BC1_UNORM, Rgb, U8);
    bench_decoder_with(c, DxgiFormat::BC1_UNORM, Rgb, U8, |c| {
        c.size = (4095, 4095).into();
    });
    bench_decoder(c, DxgiFormat::BC4_UNORM, Grayscale, U8);
    bench_decoder(c, DxgiFormat::BC4_SNORM, Grayscale, U8);
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = random_bc7_modes;
    });
}

pub fn bc7_modes(c: &mut Criterion) {
    use Channels::*;
    use Precision::*;

    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 0);
        };
        c.name = "mode 0";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 1);
        };
        c.name = "mode 1";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 2);
        };
        c.name = "mode 2";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 3);
        };
        c.name = "mode 3";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 4);
        };
        c.name = "mode 4";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 5);
        };
        c.name = "mode 5";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 6);
        };
        c.name = "mode 6";
        c.size = (1024, 1024).into();
    });
    bench_decoder_with(c, DxgiFormat::BC7_UNORM, Rgba, U8, |c| {
        c.data_modifier = |data| {
            set_bc7_modes(data, 7);
        };
        c.name = "mode 7";
        c.size = (1024, 1024).into();
    });
}

criterion_group!(benches, uncompressed, bc7_modes);
criterion_main!(benches);
