use ddsd::*;
use util::{test_data_dir, Image, WithPrecision};

mod util;

const FORMATS: &[EncodeFormat] = &[
    // uncompressed formats
    EncodeFormat::R8G8B8_UNORM,
    EncodeFormat::B8G8R8_UNORM,
    EncodeFormat::R8G8B8A8_UNORM,
    EncodeFormat::R8G8B8A8_SNORM,
    EncodeFormat::B8G8R8A8_UNORM,
    EncodeFormat::B8G8R8X8_UNORM,
    EncodeFormat::B5G6R5_UNORM,
    EncodeFormat::B5G5R5A1_UNORM,
    EncodeFormat::B4G4R4A4_UNORM,
    EncodeFormat::A4B4G4R4_UNORM,
    EncodeFormat::R8_SNORM,
    EncodeFormat::R8_UNORM,
    EncodeFormat::R8G8_UNORM,
    EncodeFormat::R8G8_SNORM,
    EncodeFormat::A8_UNORM,
    EncodeFormat::R16_UNORM,
    EncodeFormat::R16_SNORM,
    EncodeFormat::R16G16_UNORM,
    EncodeFormat::R16G16_SNORM,
    EncodeFormat::R16G16B16A16_UNORM,
    EncodeFormat::R16G16B16A16_SNORM,
    EncodeFormat::R10G10B10A2_UNORM,
    EncodeFormat::R11G11B10_FLOAT,
    EncodeFormat::R9G9B9E5_SHAREDEXP,
    EncodeFormat::R16_FLOAT,
    EncodeFormat::R16G16_FLOAT,
    EncodeFormat::R16G16B16A16_FLOAT,
    EncodeFormat::R32_FLOAT,
    EncodeFormat::R32G32_FLOAT,
    EncodeFormat::R32G32B32_FLOAT,
    EncodeFormat::R32G32B32A32_FLOAT,
    EncodeFormat::R10G10B10_XR_BIAS_A2_UNORM,
    EncodeFormat::AYUV,
    EncodeFormat::Y410,
    EncodeFormat::Y416,
    // sub-sampled formats
    EncodeFormat::R1_UNORM,
    EncodeFormat::R8G8_B8G8_UNORM,
    EncodeFormat::G8R8_G8B8_UNORM,
    EncodeFormat::UYVY,
    EncodeFormat::YUY2,
    EncodeFormat::Y210,
    EncodeFormat::Y216,
    // block compression formats
    EncodeFormat::BC1_UNORM,
    EncodeFormat::BC2_UNORM,
    EncodeFormat::BC2_UNORM_PREMULTIPLIED_ALPHA,
    EncodeFormat::BC3_UNORM,
    EncodeFormat::BC3_UNORM_PREMULTIPLIED_ALPHA,
    EncodeFormat::BC4_UNORM,
    EncodeFormat::BC4_SNORM,
    EncodeFormat::BC5_UNORM,
    EncodeFormat::BC5_SNORM,
    EncodeFormat::BC6H_UF16,
    EncodeFormat::BC6H_SF16,
    EncodeFormat::BC7_UNORM,
];

fn create_header(size: Size, format: EncodeFormat) -> Header {
    if let Ok(dxgi_format) = format.try_into() {
        Header::new_image(size.width, size.height, dxgi_format)
    } else if let Ok(format) = format.try_into() {
        Dx9Header::new_image(size.width, size.height, format).into()
    } else {
        unreachable!("unsupported format: {:?}", format);
    }
}
fn write_dds_header(size: Size, format: EncodeFormat) -> Vec<u8> {
    let header = create_header(size, format);

    let mut output = Vec::new();
    output.extend_from_slice(&Header::MAGIC);
    header.to_raw().write(&mut output).unwrap();

    output
}
fn encode_image<T: WithPrecision + util::Castable, W: std::io::Write>(
    image: &Image<T>,
    format: EncodeFormat,
    writer: &mut W,
    options: &EncodeOptions,
) -> Result<(), EncodeError> {
    format.encode(writer, image.size, image.color(), image.as_bytes(), options)
}
fn encode_decode(format: EncodeFormat, options: &EncodeOptions, image: &Image<f32>) -> Image<f32> {
    // encode
    let mut encoded = Vec::new();
    encode_image(image, format, &mut encoded, options).unwrap();

    // decode
    let header = create_header(image.size, format);
    let decode_format = DecodeFormat::from_header(&header).unwrap();
    let mut output = vec![0_f32; image.size.pixels() as usize * image.channels.count() as usize];
    decode_format
        .decode_f32(
            &mut encoded.as_slice(),
            image.size,
            image.channels,
            &mut output,
        )
        .unwrap();

    Image {
        size: image.size,
        channels: image.channels,
        data: output,
    }
}

#[test]
fn encode_base() {
    let base_u8 = util::read_png_u8(&util::test_data_dir().join("base.png")).unwrap();
    assert!(base_u8.channels == Channels::Rgba);
    let base_u16 = base_u8.to_u16();
    let base_f32 = base_u8.to_f32();

    let test = |format: EncodeFormat| -> Result<(), Box<dyn std::error::Error>> {
        let mut output = write_dds_header(base_u8.size, format);

        let options = EncodeOptions::default();

        // and now the image data
        if format.precision() == Precision::U16 {
            encode_image(&base_u16, format, &mut output, &options)?;
        } else if format.precision() == Precision::F32 {
            encode_image(&base_f32, format, &mut output, &options)?;
        } else {
            encode_image(&base_u8, format, &mut output, &options)?;
        }

        // write to disk
        let name = format!("{:?}.dds", format);
        let path = test_data_dir().join("output-encode/base").join(&name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &output)?;

        Ok(())
    };

    let mut failed_count = 0;
    for format in FORMATS.iter().copied() {
        if let Err(e) = test(format) {
            eprintln!("Failed to encode {:?}: {}", format, e);
            failed_count += 1;
        }
    }
    if failed_count > 0 {
        panic!("{} tests failed", failed_count);
    }
}

#[test]
fn encode_dither() {
    let test = |format: EncodeFormat,
                image: &Image<f32>,
                name: &str|
     -> Result<(), Box<dyn std::error::Error>> {
        let mut output = write_dds_header(image.size, format);

        let mut options = EncodeOptions::default();
        options.dither = DitheredChannels::All;
        encode_image(image, format, &mut output, &options)?;

        // write to disk
        let name = format!("{:?} {}.dds", format, name);
        let path = test_data_dir().join("output-encode/dither").join(&name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &output)?;

        Ok(())
    };

    let base = util::read_png_u8(&util::test_data_dir().join("base.png"))
        .unwrap()
        .to_f32();
    let twirl = util::read_png_u8(&util::test_data_dir().join("color-twirl.png"))
        .unwrap()
        .to_f32();

    let ignore = [
        EncodeFormat::BC4_SNORM,
        EncodeFormat::BC5_UNORM,
        EncodeFormat::BC5_SNORM,
    ];

    let mut failed_count = 0;
    for format in FORMATS
        .iter()
        .copied()
        .filter(|f| f.supports_dither() != DitheredChannels::None)
        .filter(|f| !ignore.contains(f))
    {
        let dither = format.supports_dither();

        if let Err(e) = test(format, &base, "base") {
            eprintln!("Failed to encode {:?}: {}", format, e);
            failed_count += 1;
        }

        if dither != DitheredChannels::AlphaOnly {
            if let Err(e) = test(format, &twirl, "twirl") {
                eprintln!("Failed to encode {:?}: {}", format, e);
                failed_count += 1;
            }
        }
    }
    if failed_count > 0 {
        panic!("{} tests failed", failed_count);
    }
}

#[test]
fn encode_measure_quality() {
    let base = util::read_png_u8(&util::test_data_dir().join("base.png"))
        .unwrap()
        .to_f32();
    let twirl = util::read_png_u8(&util::test_data_dir().join("color-twirl.png"))
        .unwrap()
        .to_f32();

    struct TestImage<'a> {
        name: &'a str,
        image: &'a Image<f32>,
    }
    impl<'a> TestImage<'a> {
        fn new(name: &'a str, image: &'a Image<f32>) -> Self {
            Self { name, image }
        }
    }
    struct TestCase<'a> {
        format: EncodeFormat,
        options: EncodeOptions,
        images: &'a [TestImage<'a>],
    }

    let cases = [
        TestCase {
            format: EncodeFormat::BC4_UNORM,
            options: EncodeOptions::default(),
            images: &[
                TestImage::new("base", &base),
                TestImage::new("twirl", &twirl),
            ],
        },
        TestCase {
            format: EncodeFormat::BC4_UNORM,
            options: EncodeOptions {
                dither: DitheredChannels::All,
                ..Default::default()
            },
            images: &[
                TestImage::new("base", &base),
                TestImage::new("twirl", &twirl),
            ],
        },
    ];

    let collect_info = |case: &TestCase| -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        for image in case.images {
            output.push_str(&format!("{}\n", image.name));

            let image = image.image.to_channels(case.format.channels());
            let encoded = encode_decode(case.format, &case.options, &image);
            let metrics = util::measure_compression_quality(&image, &encoded);

            let mut table = PrettyTable::new(metrics.len() + 1, 4);
            table.set(0, 1, "    ↑PSNR");
            table.set(0, 2, "    ↑PSNR blur");
            table.set(0, 3, "    ↓Region");
            for (i, m) in metrics.iter().enumerate() {
                table.set(i + 1, 0, format!("{:?}", m.channel));
                table.set(i + 1, 1, format!("{:.3}", m.psnr));
                table.set(i + 1, 2, format!("{:.3}", m.psnr_blur));
                table.set(i + 1, 3, format!("{:.5}", m.region_error * 255.));
            }
            table.print(&mut output);
        }

        Ok(output)
    };

    let mut output = String::new();
    for case in cases {
        output.push_str(&format!("{:?}\n", case.format));
        output.push_str(&format!("{:#?}\n", case.options));

        let info = match collect_info(&case) {
            Ok(info) => info,
            Err(e) => format!("Error: {}", e),
        };

        for line in info.lines() {
            if line.is_empty() {
                output.push('\n');
            } else {
                output.push_str(&format!("    {}\n", line.trim_ascii_end()));
            }
        }

        output.push('\n');
        output.push('\n');
        output.push('\n');
    }

    util::compare_snapshot_text(&util::test_data_dir().join("encode_quality.txt"), &output);
}

struct PrettyTable {
    cells: Vec<String>,
    width: usize,
    height: usize,
}
impl PrettyTable {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![String::new(); width * height],
            width,
            height,
        }
    }

    pub fn get(&self, x: usize, y: usize) -> &str {
        &self.cells[y * self.width + x]
    }
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut String {
        &mut self.cells[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, value: impl Into<String>) {
        *self.get_mut(x, y) = value.into();
    }

    pub fn print(&self, out: &mut String) {
        let column_width: Vec<usize> = (0..self.width)
            .map(|x| {
                (0..self.height)
                    .map(|y| self.get(x, y).chars().count())
                    .max()
                    .unwrap()
            })
            .collect();

        for y in 0..self.height {
            #[allow(clippy::needless_range_loop)]
            for x in 0..self.width {
                let cell = self.get(x, y);
                out.push_str(cell);
                for _ in 0..column_width[x] - cell.chars().count() {
                    out.push(' ');
                }
                out.push_str("  ");
            }
            out.push('\n');
        }
    }
}
