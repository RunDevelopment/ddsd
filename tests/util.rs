use ddsd::*;
use std::{fs::File, path::PathBuf};
use zerocopy::{FromBytes, Immutable, IntoBytes, Ref};
use Precision::*;

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

pub struct Image<T> {
    pub data: Vec<T>,
    pub channels: Channels,
    pub size: Size,
}
impl<T> Image<T> {
    pub fn stride(&self) -> usize {
        self.size.width as usize * self.channels.count() as usize * std::mem::size_of::<T>()
    }
}
impl<T: WithPrecision> Image<T> {
    pub fn precision(&self) -> Precision {
        T::PRECISION
    }
    pub fn color(&self) -> ColorFormat {
        ColorFormat::new(self.channels, T::PRECISION)
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
    select_channels: impl FnOnce(SupportedFormat) -> Channels,
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
    select_channels: impl FnOnce(SupportedFormat) -> Channels,
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

pub fn compare_snapshot_png_u8(
    png_path: &PathBuf,
    image: &Image<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (channels, color) = to_png_compatible_channels(image.channels);
    assert!(channels == image.channels);

    // compare to PNG
    let png_exists = png_path.exists();
    if png_exists {
        let png_decoder = png::Decoder::new(File::open(png_path)?);
        let mut png_reader = png_decoder.read_info()?;
        let (mut png_color, png_bits) = png_reader.output_color_type();
        if png_bits != png::BitDepth::Eight {
            return Err("Output PNG is not 8-bit, which shouldn't happen.".into());
        }

        let mut png_image_data = vec![0; png_reader.output_buffer_size()];
        png_reader.next_frame(&mut png_image_data)?;
        png_reader.finish()?;

        if color == png::ColorType::Rgba && png_color == png::ColorType::Rgb {
            // convert to RGBA
            png_image_data = convert_channels(&png_image_data, Channels::Rgb, Channels::Rgba);
            png_color = png::ColorType::Rgba;
        }

        if png_color != color {
            eprintln!("Color mismatch: {:?} != {:?}", png_color, color);
        } else {
            assert!(png_image_data.len() == image.data.len());
            if png_image_data == image.data {
                // all good
                return Ok(());
            }
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
    fn write_u32(w: &mut impl std::io::Write, x: u32) -> std::io::Result<()> {
        w.write_all(&x.to_le_bytes())
    }

    w.write_all(&Header::MAGIC)?;
    write_u32(w, 124)?;
    write_u32(w, DdsFlags::REQUIRED.bits())?;
    write_u32(w, size.height)?; // height
    write_u32(w, size.width)?; // width
    write_u32(w, 0)?; // pitch_or_linear_size
    write_u32(w, 0)?; // depth
    write_u32(w, 0)?; // mip_map_count
    for _ in 0..11 {
        write_u32(w, 0)?;
    }
    write_u32(w, 32)?; // size
    write_u32(w, PixelFormatFlags::FOURCC.bits())?; // flags
    write_u32(w, FourCC::DX10.into())?; // four_cc
    write_u32(w, 0)?; // rgb_bit_count
    write_u32(w, 0)?; // r_bit_mask
    write_u32(w, 0)?; // g_bit_mask
    write_u32(w, 0)?; // b_bit_mask
    write_u32(w, 0)?; // a_bit_mask
    write_u32(w, DdsCaps::TEXTURE.bits())?; // caps
    write_u32(w, 0)?; // caps2
    write_u32(w, 0)?; // caps3
    write_u32(w, 0)?; // caps4
    write_u32(w, 0)?; // reserved2
    write_u32(w, format.into())?; // dxgiFormat
    write_u32(w, ResourceDimension::Texture2D.into())?; // resource_dimension
    write_u32(w, 0)?; // misc_flag
    write_u32(w, 1)?; // array_size
    write_u32(w, 0)?; // misc_flags2

    Ok(())
}
