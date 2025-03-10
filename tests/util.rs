use ddsd::*;
use rand::SeedableRng;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use zerocopy::{FromBytes, Immutable, IntoBytes, Ref};
use Precision::*;

pub fn create_rng() -> impl rand::Rng {
    rand_chacha::ChaChaRng::seed_from_u64(123456789)
}

pub trait Castable: FromBytes + IntoBytes + Immutable {}
impl<T: FromBytes + IntoBytes + Immutable> Castable for T {}
pub fn from_bytes<T: Castable>(bytes: &[u8]) -> Option<&[T]> {
    Ref::from_bytes(bytes).ok().map(Ref::into_ref)
}
pub fn from_bytes_mut<T: Castable>(bytes: &mut [u8]) -> Option<&mut [T]> {
    Ref::from_bytes(bytes).ok().map(Ref::into_mut)
}
pub fn as_bytes_mut<T: Castable>(buffer: &mut [T]) -> &mut [u8] {
    buffer.as_mut_bytes()
}
pub fn as_bytes<T: Castable>(buffer: &[T]) -> &[u8] {
    buffer.as_bytes()
}
pub fn cast_slice<T: Castable, U: Castable>(data: &[T]) -> &[U] {
    let data_bytes = as_bytes(data);
    from_bytes(data_bytes).unwrap()
}
pub fn cast_slice_mut<T: Castable, U: Castable>(data: &mut [T]) -> &mut [U] {
    let data_bytes = as_bytes_mut(data);
    from_bytes_mut(data_bytes).unwrap()
}

pub fn is_ci() -> bool {
    std::env::var("CI").is_ok()
}

pub fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test-data");
    path
}

pub fn example_dds_files() -> Vec<PathBuf> {
    example_dds_files_in("**")
}
pub fn example_dds_files_in(parent_dir: &str) -> Vec<PathBuf> {
    glob::glob(
        test_data_dir()
            .join(format!("images/{parent_dir}/*.dds"))
            .to_str()
            .unwrap(),
    )
    .expect("Failed to read glob pattern")
    .map(|x| x.unwrap())
    // ignore files starting with "_"
    .filter(|x| !x.file_name().unwrap().to_str().unwrap().starts_with('_'))
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image<T> {
    pub data: Vec<T>,
    pub channels: Channels,
    pub size: Size,
}
impl<T> Image<T> {
    pub fn stride(&self) -> usize {
        self.size.width as usize * self.channels.count() as usize * std::mem::size_of::<T>()
    }

    pub fn as_bytes(&self) -> &[u8]
    where
        T: Castable,
    {
        as_bytes(&self.data)
    }

    pub fn precision(&self) -> Precision
    where
        T: WithPrecision,
    {
        T::PRECISION
    }
    pub fn color(&self) -> ColorFormat
    where
        T: WithPrecision,
    {
        ColorFormat::new(self.channels, T::PRECISION)
    }

    pub fn to_channels(&self, channels: Channels) -> Image<T>
    where
        T: Copy + Default + Castable + Norm,
    {
        Image {
            data: convert_channels(&self.data, self.channels, channels),
            channels,
            size: self.size,
        }
    }
}
impl Image<u8> {
    pub fn to_u16(&self) -> Image<u16> {
        Image {
            data: self.data.iter().map(|&x| x as u16 * 257).collect(),
            channels: self.channels,
            size: self.size,
        }
    }
    pub fn to_f32(&self) -> Image<f32> {
        Image {
            data: self.data.iter().map(|&x| x as f32 / 255.0).collect(),
            channels: self.channels,
            size: self.size,
        }
    }
}

pub trait WithPrecision {
    const PRECISION: Precision;
}
impl WithPrecision for u8 {
    const PRECISION: Precision = U8;
}
impl WithPrecision for u16 {
    const PRECISION: Precision = U16;
}
impl WithPrecision for f32 {
    const PRECISION: Precision = F32;
}

pub fn read_dds<T: WithPrecision + Default + Copy + Castable>(
    dds_path: &PathBuf,
) -> Result<(Image<T>, DdsDecoder), Box<dyn std::error::Error>> {
    read_dds_with_channels_select(dds_path, |f| f.channels())
}
pub fn read_dds_with_channels<T: WithPrecision + Default + Copy + Castable>(
    dds_path: &PathBuf,
    channels: Channels,
) -> Result<(Image<T>, DdsDecoder), Box<dyn std::error::Error>> {
    read_dds_with_channels_select(dds_path, |_| channels)
}
pub fn read_dds_with_channels_select<T: WithPrecision + Default + Copy + Castable>(
    dds_path: &PathBuf,
    select_channels: impl FnOnce(DecodeFormat) -> Channels,
) -> Result<(Image<T>, DdsDecoder), Box<dyn std::error::Error>> {
    let mut file = File::open(dds_path)?;

    let mut options = Options::default();
    options.permissive = true;
    options.file_len = Some(file.metadata()?.len());

    decode_dds_with_channels_select(&options, &mut file, select_channels)
}

pub fn decode_dds_with_channels<T: WithPrecision + Default + Copy + Castable>(
    options: &Options,
    reader: impl std::io::Read,
    channels: Channels,
) -> Result<(Image<T>, DdsDecoder), Box<dyn std::error::Error>> {
    decode_dds_with_channels_select(options, reader, |_| channels)
}
pub fn decode_dds_with_channels_select<T: WithPrecision + Default + Copy + Castable>(
    options: &Options,
    mut reader: impl std::io::Read,
    select_channels: impl FnOnce(DecodeFormat) -> Channels,
) -> Result<(Image<T>, DdsDecoder), Box<dyn std::error::Error>> {
    let decoder = DdsDecoder::new_with(&mut reader, &options)?;
    let size = decoder.header().size();
    let format = decoder.format();
    if !format.supports_precision(T::PRECISION) {
        return Err(format!("Format does not support decoding as {:?}", T::PRECISION).into());
    }

    let channels = select_channels(format);
    if !format.supports_channels(channels) {
        // can't read in a way PNG likes
        return Err("Unsupported channels".into());
    }

    let mut image_data = vec![T::default(); size.pixels() as usize * channels.count() as usize];
    let image_data_bytes: &mut [u8] = as_bytes_mut(&mut image_data);
    format.decode(
        &mut reader,
        size,
        ColorFormat::new(channels, T::PRECISION),
        image_data_bytes,
    )?;

    let image = Image {
        data: image_data,
        channels,
        size,
    };
    Ok((image, decoder))
}

pub fn read_dds_png_compatible(
    dds_path: &PathBuf,
) -> Result<(Image<u8>, DdsDecoder), Box<dyn std::error::Error>> {
    read_dds_with_channels_select(dds_path, |f| to_png_compatible_channels(f.channels()).0)
}

pub fn read_dds_rect_as_u8(
    dds_path: &PathBuf,
    rect: Rect,
) -> Result<(Image<u8>, DdsDecoder), Box<dyn std::error::Error>> {
    // read dds
    let mut file = File::open(dds_path)?;

    let decoder = DdsDecoder::new(&mut file)?;
    let size = decoder.header().size();
    let format = decoder.format();
    if !format.supports_precision(Precision::U8) {
        return Err("Format does not support decoding as U8".into());
    }

    let channels = to_png_compatible_channels(format.channels()).0;
    if !format.supports_channels(channels) {
        // can't read in a way PNG likes
        return Err("Unsupported channels".into());
    }

    let color = ColorFormat::new(channels, U8);
    let bpp = color.bytes_per_pixel() as usize;
    let mut image_data = vec![0_u8; rect.size().pixels() as usize * bpp];
    format.decode_rect(
        &mut file,
        size,
        rect,
        color,
        &mut image_data,
        rect.width as usize * bpp,
    )?;

    let image = Image {
        data: image_data,
        channels,
        size: rect.size(),
    };
    Ok((image, decoder))
}

pub fn to_png_compatible_channels(channels: Channels) -> (Channels, png::ColorType) {
    match channels {
        Channels::Grayscale => (Channels::Grayscale, png::ColorType::Grayscale),
        Channels::Alpha => (Channels::Rgba, png::ColorType::Rgba),
        Channels::Rgb => (Channels::Rgb, png::ColorType::Rgb),
        Channels::Rgba => (Channels::Rgba, png::ColorType::Rgba),
    }
}

pub fn read_png_u8(png_path: &Path) -> Result<Image<u8>, Box<dyn std::error::Error>> {
    let png_decoder = png::Decoder::new(File::open(png_path)?);
    let mut png_reader = png_decoder.read_info()?;
    let (color, bits) = png_reader.output_color_type();

    if bits != png::BitDepth::Eight {
        return Err("Output PNG is not 8-bit, which shouldn't happen.".into());
    }
    let channels = match color {
        png::ColorType::Grayscale => Channels::Grayscale,
        png::ColorType::Rgb => Channels::Rgb,
        png::ColorType::Rgba => Channels::Rgba,
        _ => return Err("Unsupported PNG color type".into()),
    };

    let mut png_image_data = vec![0; png_reader.output_buffer_size()];
    png_reader.next_frame(&mut png_image_data)?;
    png_reader.finish()?;

    Ok(Image {
        data: png_image_data,
        channels,
        size: Size::new(png_reader.info().width, png_reader.info().height),
    })
}

pub fn compare_snapshot_png_u8(
    png_path: &PathBuf,
    image: &Image<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (channels, color) = to_png_compatible_channels(image.channels);
    assert!(channels == image.channels);

    // compare to PNG
    let png_exists = png_path.exists();
    if png_exists {
        let mut png = read_png_u8(png_path)?;

        if image.channels == Channels::Rgba && png.channels == Channels::Rgb {
            // convert to RGBA
            png.data = convert_channels(&png.data, Channels::Rgb, Channels::Rgba);
            png.channels = Channels::Rgba;
        }

        if png.data == image.data {
            // all good
            return Ok(());
        }

        if image.channels != png.channels {
            eprintln!("Color mismatch: {:?} != {:?}", png.channels, image.channels);
        }
    }

    // write output PNG
    if !is_ci() {
        println!("Writing PNG: {:?}", png_path);
        let mut output = Vec::new();
        let mut encoder = png::Encoder::new(&mut output, image.size.width, image.size.height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image.data)?;
        writer.finish()?;

        std::fs::create_dir_all(png_path.parent().unwrap())?;
        std::fs::write(png_path, output)?;
    }

    if !png_exists {
        return Err("Output PNG didn't exist".into());
    }
    Err("Output PNG didn't match".into())
}

pub fn compare_snapshot_dds_f32(
    dds_path: &PathBuf,
    image: &Image<f32>,
) -> Result<(), Box<dyn std::error::Error>> {
    // compare to DDS
    let dds_exists = dds_path.exists();
    if dds_exists {
        let mut file = File::open(dds_path)?;
        let dds_decoder = DdsDecoder::new(&mut file)?;
        let size = dds_decoder.header().size();

        let mut dds_image_data =
            vec![0.0_f32; size.pixels() as usize * image.channels.count() as usize];
        dds_decoder
            .format()
            .decode_f32(&mut file, size, image.channels, &mut dds_image_data)?;

        assert!(dds_image_data.len() == image.data.len());
        if dds_image_data == image.data {
            // all good
            return Ok(());
        }
    }

    // write output DDS
    if !is_ci() {
        println!("Writing DDS: {:?}", dds_path);

        let mut output = Vec::new();
        write_simple_dds_header(
            &mut output,
            image.size,
            match image.channels {
                Channels::Grayscale => DxgiFormat::R32_FLOAT,
                Channels::Alpha => DxgiFormat::R32_FLOAT,
                Channels::Rgb => DxgiFormat::R32G32B32_FLOAT,
                Channels::Rgba => DxgiFormat::R32G32B32A32_FLOAT,
            },
        )?;

        // convert to LE
        let mut data = image.data.clone();
        let data_u32: &mut [u32] = cast_slice_mut(&mut data);
        data_u32.iter_mut().for_each(|x| *x = x.to_le());
        output.extend_from_slice(as_bytes(&data));

        std::fs::create_dir_all(dds_path.parent().unwrap())?;
        std::fs::write(dds_path, output)?;
    }

    if !dds_exists {
        return Err("Output DDS didn't exist".into());
    }
    Err("Output DDS didn't match".into())
}

pub trait Norm {
    const NORM_ONE: Self;
    const NORM_ZERO: Self;
}
impl Norm for u8 {
    const NORM_ONE: Self = u8::MAX;
    const NORM_ZERO: Self = 0;
}
impl Norm for u16 {
    const NORM_ONE: Self = u16::MAX;
    const NORM_ZERO: Self = 0;
}
impl Norm for f32 {
    const NORM_ONE: Self = 1.0;
    const NORM_ZERO: Self = 0.0;
}

pub fn convert_channels<T>(data: &[T], from: Channels, to: Channels) -> Vec<T>
where
    T: Copy + Default + Castable + Norm,
{
    if from == to {
        return data.to_vec();
    }

    fn convert<const N: usize, const M: usize, T>(
        data: &[T],
        f: impl Fn([T; N]) -> [T; M],
    ) -> Vec<T>
    where
        T: Copy + Default + Castable,
    {
        let pixels = data.len() / N;
        let mut result: Vec<T> = vec![Default::default(); pixels * M];

        let data_n: &[[T; N]] = cast_slice(data);
        let result_m: &mut [[T; M]] = cast_slice_mut(&mut result);

        for (i, o) in data_n.iter().zip(result_m.iter_mut()) {
            *o = f(*i);
        }

        result
    }

    match (from, to) {
        // already handled
        (Channels::Grayscale, Channels::Grayscale)
        | (Channels::Alpha, Channels::Alpha)
        | (Channels::Rgb, Channels::Rgb)
        | (Channels::Rgba, Channels::Rgba) => unreachable!(),

        (Channels::Grayscale, Channels::Alpha) => convert(data, |[_]| [T::NORM_ONE]),
        (Channels::Grayscale, Channels::Rgb) => convert(data, |[g]| [g, g, g]),
        (Channels::Grayscale, Channels::Rgba) => convert(data, |[g]| [g, g, g, T::NORM_ONE]),
        (Channels::Alpha, Channels::Grayscale) => convert(data, |[_]| [T::NORM_ZERO]),
        (Channels::Alpha, Channels::Rgb) => {
            convert(data, |[_]| [T::NORM_ZERO, T::NORM_ZERO, T::NORM_ZERO])
        }
        (Channels::Alpha, Channels::Rgba) => {
            convert(data, |[a]| [T::NORM_ZERO, T::NORM_ZERO, T::NORM_ZERO, a])
        }
        (Channels::Rgb, Channels::Grayscale) => convert(data, |[r, _, _]| [r]),
        (Channels::Rgb, Channels::Alpha) => convert(data, |[_, _, _]| [T::NORM_ONE]),
        (Channels::Rgb, Channels::Rgba) => convert(data, |[r, g, b]| [r, g, b, T::NORM_ONE]),
        (Channels::Rgba, Channels::Grayscale) => convert(data, |[r, _, _, _]| [r]),
        (Channels::Rgba, Channels::Alpha) => convert(data, |[_, _, _, a]| [a]),
        (Channels::Rgba, Channels::Rgb) => convert(data, |[r, g, b, _]| [r, g, b]),
    }
}

pub fn write_simple_dds_header(
    w: &mut impl std::io::Write,
    size: Size,
    format: DxgiFormat,
) -> std::io::Result<()> {
    let mut header = Header::new_image(size.width, size.height, format);
    header.dx10_mut().unwrap().alpha_mode = AlphaMode::Unknown;

    w.write_all(&Header::MAGIC)?;
    header.to_raw().write(w)?;

    Ok(())
}

pub fn compare_snapshot_text(snapshot_file: &Path, text: &str) {
    let text = text.replace("\r\n", "\n");

    // compare to snapshot
    let file_exists = snapshot_file.exists();
    let mut native_line_ends = "\n";

    if file_exists {
        let mut snapshot = std::fs::read_to_string(snapshot_file).unwrap();
        if snapshot.contains("\r\n") {
            native_line_ends = "\r\n";
            snapshot = snapshot.replace("\r\n", "\n");
        }

        if text.trim() == snapshot.trim() {
            // all ok
            return;
        }
    }

    // write snapshot
    if !is_ci() {
        println!("Writing snapshot: {:?}", snapshot_file);

        std::fs::create_dir_all(snapshot_file.parent().unwrap()).unwrap();
        std::fs::write(snapshot_file, text.replace("\n", native_line_ends)).unwrap();
    }

    if !file_exists {
        panic!("Snapshot file not found: {:?}", snapshot_file);
    } else {
        panic!("Snapshot differs from expected.");
    }
}

pub fn pretty_print_header(out: &mut String, header: &Header) {
    out.push_str("Header:\n");
    if let Some(d) = header.depth() {
        out.push_str(&format!(
            "    w/h/d: {:?} x {:?} x {:?}\n",
            header.width(),
            header.height(),
            d
        ));
    } else {
        out.push_str(&format!(
            "    w/h: {:?} x {:?}\n",
            header.width(),
            header.height()
        ));
    }
    out.push_str(&format!("    mipmap_count: {:?}\n", header.mipmap_count()));
    match header {
        Header::Dx9(dx9) => {
            if !dx9.caps2.is_empty() {
                out.push_str(&format!("    caps2: {:?}\n", dx9.caps2));
            }

            match &dx9.pixel_format {
                Dx9PixelFormat::FourCC(four_cc) => {
                    out.push_str(&format!("    format: {:?}\n", four_cc));
                }
                Dx9PixelFormat::Mask(pixel_format) => {
                    out.push_str("    format: masked\n");
                    out.push_str(&format!("        flags: {:?}\n", pixel_format.flags));
                    out.push_str(&format!(
                        "        rgb_bit_count: {:?}\n",
                        pixel_format.rgb_bit_count
                    ));
                    out.push_str(&format!(
                        "        bit_mask: r:0x{:x} g:0x{:x} b:0x{:x} a:0x{:x}\n",
                        pixel_format.r_bit_mask,
                        pixel_format.g_bit_mask,
                        pixel_format.b_bit_mask,
                        pixel_format.a_bit_mask
                    ));
                }
            }
        }
        Header::Dx10(dx10) => {
            out.push_str(&format!("    DX10: {:?}\n", dx10.resource_dimension));
            out.push_str(&format!("        dxgi_format: {:?}\n", dx10.dxgi_format));
            if !dx10.misc_flag.is_empty() {
                out.push_str(&format!("        misc_flag: {:?}\n", dx10.misc_flag));
            }
            if dx10.array_size != 1 {
                out.push_str(&format!("        array_size: {:?}\n", dx10.array_size));
            }
            if dx10.alpha_mode != AlphaMode::Unknown {
                out.push_str(&format!("        alpha_mode: {:?}\n", dx10.alpha_mode));
            }
        }
    };
}

pub fn pretty_print_raw_header(out: &mut String, raw: &RawHeader) {
    out.push_str("Raw Header:\n");

    if raw.size != 124 {
        out.push_str(&format!("    size: {:?}\n", raw.size));
    }
    out.push_str(&format!("    flags: {:?}\n", raw.flags));

    if raw.flags.contains(DdsFlags::DEPTH) {
        out.push_str(&format!(
            "    w/h/d: {:?} x {:?} x {:?}\n",
            raw.width, raw.height, raw.depth
        ));
    } else {
        out.push_str(&format!(
            "    w/h: {:?} x {:?} (x {:?})\n",
            raw.width, raw.height, raw.depth
        ));
    }

    let size = raw.pitch_or_linear_size;
    if raw.flags.contains(DdsFlags::PITCH) && !raw.flags.contains(DdsFlags::LINEAR_SIZE) {
        out.push_str(&format!("    pitch: {:?}\n", size));
    } else if !raw.flags.contains(DdsFlags::PITCH) && raw.flags.contains(DdsFlags::LINEAR_SIZE) {
        out.push_str(&format!("    linear_size: {:?}\n", size));
    } else {
        out.push_str(&format!("    pitch_or_linear_size: {:?}\n", size));
    }

    out.push_str(&format!("    mipmap_count: {:?}", raw.mipmap_count));
    if !raw.flags.contains(DdsFlags::MIPMAP_COUNT) {
        out.push_str("  (not specified)");
    }
    out.push('\n');

    if raw.reserved1.iter().any(|&x| x != 0) {
        out.push_str("    reserved1:\n");
        let zero_prefix = raw.reserved1.iter().take_while(|&&x| x == 0).count();
        if zero_prefix > 0 {
            out.push_str(&format!("        0..={}: 0\n", zero_prefix - 1));
        }
        for i in zero_prefix..raw.reserved1.len() {
            out.push_str(&format!("           {:>2}: ", i));

            let n = raw.reserved1[i];
            let bytes = n.to_le_bytes();

            if bytes.iter().all(|x| x.is_ascii_alphanumeric()) {
                for byte in bytes {
                    out.push(byte as char);
                }
                out.push_str(" (ASCII)");
            } else {
                out.push_str(&format!("{:#010X} {}", n, n));
            }

            out.push('\n');
        }
    }

    if raw.pixel_format.flags == PixelFormatFlags::FOURCC
        && raw.pixel_format.rgb_bit_count == 0
        && raw.pixel_format.r_bit_mask == 0
        && raw.pixel_format.g_bit_mask == 0
        && raw.pixel_format.b_bit_mask == 0
        && raw.pixel_format.a_bit_mask == 0
    {
        out.push_str(&format!(
            "    pixel_format: {:?}\n",
            raw.pixel_format.four_cc
        ));
    } else {
        out.push_str("    pixel_format:\n");
        out.push_str(&format!("        flags: {:?}\n", raw.pixel_format.flags));
        if raw.pixel_format.four_cc != FourCC::NONE {
            out.push_str(&format!(
                "        four_cc: {:?}\n",
                raw.pixel_format.four_cc
            ));
        }
        out.push_str(&format!(
            "        rgb_bit_count: {:?}\n",
            raw.pixel_format.rgb_bit_count
        ));
        out.push_str(&format!(
            "        bit_mask: r:0x{:x} g:0x{:x} b:0x{:x} a:0x{:x}\n",
            raw.pixel_format.r_bit_mask,
            raw.pixel_format.g_bit_mask,
            raw.pixel_format.b_bit_mask,
            raw.pixel_format.a_bit_mask
        ));
    }

    out.push_str(&format!("    caps: {:?}", raw.caps));
    if !raw.flags.contains(DdsFlags::CAPS) {
        out.push_str("  (not specified)");
    }
    out.push('\n');

    out.push_str(&format!("    caps2: {:?}\n", raw.caps2));
    if raw.caps3 != 0 || raw.caps4 != 0 || raw.reserved2 != 0 {
        out.push_str(&format!("    caps3: {:?}\n", raw.caps3));
        out.push_str(&format!("    caps4: {:?}\n", raw.caps4));
        out.push_str(&format!("    reserved2: {:?}\n", raw.reserved2));
    }

    if let Some(dx10) = &raw.dx10 {
        out.push_str("    DX10:\n");

        out.push_str("        dxgi_format: ");
        if let Ok(dxgi) = DxgiFormat::try_from(dx10.dxgi_format) {
            out.push_str(&format!("{:?}", dxgi));
        } else {
            out.push_str(&format!("{:?}", dx10.dxgi_format));
        }
        out.push('\n');

        out.push_str("        resource_dimension: ");
        if let Ok(dim) = ResourceDimension::try_from(dx10.resource_dimension) {
            out.push_str(&format!("{:?}", dim));
        } else {
            out.push_str(&format!("{:?}", dx10.resource_dimension));
        }
        out.push('\n');

        out.push_str(&format!("        misc_flag: {:?}\n", dx10.misc_flag));
        out.push_str(&format!("        array_size: {:?}\n", dx10.array_size));
        out.push_str(&format!("        misc_flags2: {:?}\n", dx10.misc_flags2));
    }
    // match &header.format {
    //     PixelFormat::FourCC(four_cc) => {
    //         out.push_str(&format!("    format: {:?}\n", four_cc));
    //     }
    //     PixelFormat::Mask(pixel_format) => {
    //         out.push_str("    format: masked\n");
    //         out.push_str(&format!("        flags: {:?}\n", pixel_format.flags));
    //         out.push_str(&format!(
    //             "        rgb_bit_count: {:?}\n",
    //             pixel_format.rgb_bit_count
    //         ));
    //         out.push_str(&format!(
    //             "        bit_mask: r:0x{:x} g:0x{:x} b:0x{:x} a:0x{:x}\n",
    //             pixel_format.r_bit_mask,
    //             pixel_format.g_bit_mask,
    //             pixel_format.b_bit_mask,
    //             pixel_format.a_bit_mask
    //         ));
    //     }
    //     PixelFormat::Dx10(dx10) => {
    //         out.push_str("    format: DX10\n");
    //         out.push_str(&format!("        dxgi_format: {:?}\n", dx10.dxgi_format));
    //         out.push_str(&format!(
    //             "        resource_dimension: {:?}\n",
    //             dx10.resource_dimension
    //         ));
    //         out.push_str(&format!("        misc_flag: {:?}\n", dx10.misc_flag));
    //         out.push_str(&format!("        array_size: {:?}\n", dx10.array_size));
    //     }
    // };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricChannel {
    L,
    R,
    G,
    B,
    A,
}
pub struct Metrics {
    pub channel: MetricChannel,
    pub psnr: f64,
    /// This the PSNR of the image after a small blur
    pub psnr_blur: f64,
    pub region_error: f64,
}
pub fn measure_compression_quality(org: &Image<f32>, compressed: &Image<f32>) -> Vec<Metrics> {
    assert!(org.size == compressed.size);
    assert!(org.channels == compressed.channels);
    assert!(org.data.len() == compressed.data.len());
    let width = org.size.width as usize;
    let height = org.size.height as usize;

    fn calculate_psnr<T, F>(org: &[T], compressed: &[T], get_value: F) -> f64
    where
        T: Copy,
        F: Fn(T) -> f64,
    {
        let mut mse = 0.0;
        for (&o, &c) in org.iter().zip(compressed.iter()) {
            let diff = get_value(o) - get_value(c);
            mse += diff * diff;
        }
        mse /= org.len() as f64;
        -10.0 * mse.log10()
    }
    fn box_blur<T, F>(image: &[T], width: usize, height: usize, get_value: F) -> Vec<f64>
    where
        T: Copy,
        F: Fn(T) -> f64,
    {
        let mut blurred: Vec<f64> = image.iter().map(|&x| get_value(x)).collect();

        const GAUSS_WEIGHTS: [f64; 5] = {
            let raw = [1.0, 4.0, 6.0, 4.0, 1.0];
            let sum = raw[0] + raw[1] + raw[2] + raw[3] + raw[4];
            [
                raw[0] / sum,
                raw[1] / sum,
                raw[2] / sum,
                raw[3] / sum,
                raw[4] / sum,
            ]
        };
        fn weigh(values: [f64; 5]) -> f64 {
            values[0] * GAUSS_WEIGHTS[0]
                + values[1] * GAUSS_WEIGHTS[1]
                + values[2] * GAUSS_WEIGHTS[2]
                + values[3] * GAUSS_WEIGHTS[3]
                + values[4] * GAUSS_WEIGHTS[4]
        }

        // Pass 1: horizontal
        for y in 0..height {
            let index_base = y * width;
            let mut prev_prev = blurred[index_base];
            let mut prev = blurred[index_base];
            let last_index = index_base + width - 1;
            for x in 0..width {
                let current = blurred[index_base + x];
                let next = blurred[last_index.min(index_base + x + 1)];
                let next_next = blurred[last_index.min(index_base + x + 2)];
                let sum = weigh([prev_prev, prev, current, next, next_next]);
                prev_prev = prev;
                prev = current;
                blurred[index_base + x] = sum;
            }
        }

        // Pass 2: vertical
        for x in 0..width {
            let mut prev_prev = blurred[x];
            let mut prev = blurred[x];
            let last_index = (height - 1) * width + x;
            for y in 0..height {
                let index = y * width + x;
                let current = blurred[index];
                let next = blurred[last_index.min(index + width)];
                let next_next = blurred[last_index.min(index + 2 * width)];
                let sum = weigh([prev_prev, prev, current, next, next_next]);
                prev_prev = prev;
                prev = current;
                blurred[index] = sum;
            }
        }

        blurred
    }
    fn calculate_metrics<T, F>(
        org: &[T],
        compressed: &[T],
        width: usize,
        height: usize,
        channel: MetricChannel,
        get_value: F,
    ) -> Metrics
    where
        T: Copy,
        F: Copy + Fn(T) -> f64,
    {
        // PSNR
        let psnr = calculate_psnr(org, compressed, get_value);

        // blurred PSNR
        let blurred_org = box_blur(org, width, height, get_value);
        let blurred_compressed = box_blur(compressed, width, height, get_value);
        let psnr_blur = calculate_psnr(&blurred_org, &blurred_compressed, |x| x);

        // region error is just the absolute average error per 4x4 region
        const REGION_SIZE: usize = 4;
        let mut region_error = 0.0;
        for region_y in 0..height / REGION_SIZE {
            for region_x in 0..width / REGION_SIZE {
                let mut region = 0.0;
                for y in 0..REGION_SIZE {
                    for x in 0..REGION_SIZE {
                        let i = (region_y * REGION_SIZE + y) * width + region_x * REGION_SIZE + x;
                        let diff = get_value(org[i]) - get_value(compressed[i]);
                        region += diff;
                    }
                }
                region_error += region.abs() / (REGION_SIZE * REGION_SIZE) as f64;
            }
        }
        region_error /= (width / REGION_SIZE * height / REGION_SIZE) as f64;

        Metrics {
            channel,
            psnr,
            psnr_blur,
            region_error,
        }
    }

    match org.channels {
        Channels::Grayscale => {
            let l = calculate_metrics(
                &org.data,
                &compressed.data,
                width,
                height,
                MetricChannel::L,
                |x| x as f64,
            );

            vec![l]
        }
        Channels::Alpha => todo!(),
        Channels::Rgb => {
            let org: &[[f32; 3]] = cast_slice(&org.data);
            let compressed: &[[f32; 3]] = cast_slice(&compressed.data);

            let l = calculate_metrics(
                org,
                compressed,
                width,
                height,
                MetricChannel::L,
                |[r, g, b]| (r * 0.25 + g * 0.6 + b * 0.15) as f64,
            );
            let r = calculate_metrics(org, compressed, width, height, MetricChannel::R, |x| {
                x[0] as f64
            });
            let g = calculate_metrics(org, compressed, width, height, MetricChannel::G, |x| {
                x[1] as f64
            });
            let b = calculate_metrics(org, compressed, width, height, MetricChannel::B, |x| {
                x[2] as f64
            });

            vec![r, g, b]
        }
        Channels::Rgba => todo!(),
    }
}
