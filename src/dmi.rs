use crate::error::{Error, Result};
use png::{Decoder, Encoder, OutputInfo, Reader};
use std::{
    fs::{create_dir_all, File},
    io::BufReader,
    path::Path,
};

byond_fn!(fn dmi_strip_metadata(path) {
    strip_metadata(path).err()
});

byond_fn!(fn dmi_create_png(path, width, height, data) {
    create_png(path, width, height, data).err()
});

byond_fn!(fn dmi_resize_png(path, width, height, resizetype) {
    let resizetype = match resizetype {
        "catmull" => image::imageops::CatmullRom,
        "gaussian" => image::imageops::Gaussian,
        "lanczos3" => image::imageops::Lanczos3,
        "nearest" => image::imageops::Nearest,
        "triangle" => image::imageops::Triangle,
        _ => image::imageops::Nearest,
    };
    resize_png(path, width, height, resizetype).err()
});

byond_fn!(fn dmi_icon_states(path) {
    read_states(path).ok()
});

fn strip_metadata(path: &str) -> Result<()> {
    let (reader, frame_info, image) = read_png(path)?;
    write_png(path, &reader, &frame_info, &image, true)
}

fn read_png(path: &str) -> Result<(Reader<File>, OutputInfo, Vec<u8>)> {
    let mut reader = Decoder::new(File::open(path)?).read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let frame_info = reader.next_frame(&mut buf)?;

    Ok((reader, frame_info, buf))
}

fn write_png(
    path: &str,
    reader: &Reader<File>,
    info: &OutputInfo,
    image: &[u8],
    strip: bool,
) -> Result<()> {
    let mut encoder = Encoder::new(File::create(path)?, info.width, info.height);
    encoder.set_color(info.color_type);
    encoder.set_depth(info.bit_depth);

    let reader_info = reader.info();
    if let Some(palette) = reader_info.palette.clone() {
        encoder.set_palette(palette);
    }

    if let Some(trns_chunk) = reader_info.trns.clone() {
        encoder.set_trns(trns_chunk);
    }

    let mut writer = encoder.write_header()?;
    // Handles zTxt chunk copying from the original image if we /don't/ want to strip it
    if !strip {
        for chunk in &reader_info.compressed_latin1_text {
            writer.write_text_chunk(chunk)?;
        }
    }
    Ok(writer.write_image_data(image)?)
}

fn create_png(path: &str, width: &str, height: &str, data: &str) -> Result<()> {
    let width = width.parse::<u32>()?;
    let height = height.parse::<u32>()?;

    let bytes = data.as_bytes();

    let mut result: Vec<u8> = Vec::new();
    for pixel in bytes.split(|&b| b == b'#').skip(1) {
        if pixel.len() != 6 && pixel.len() != 8 {
            return Err(Error::InvalidPngData);
        }
        for channel in pixel.chunks_exact(2) {
            result.push(u8::from_str_radix(std::str::from_utf8(channel)?, 16)?);
        }
        // If only RGB is provided for any pixel we also add alpha
        if pixel.len() == 6 {
            result.push(255);
        }
    }

    if let Some(fdir) = Path::new(path).parent() {
        if !fdir.is_dir() {
            create_dir_all(fdir)?;
        }
    }

    let mut encoder = Encoder::new(File::create(path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    Ok(writer.write_image_data(&result)?)
}

fn resize_png<P: AsRef<Path>>(
    path: P,
    width: &str,
    height: &str,
    resizetype: image::imageops::FilterType,
) -> std::result::Result<(), Error> {
    let width = width.parse::<u32>()?;
    let height = height.parse::<u32>()?;

    let img = image::open(path.as_ref())?;

    let newimg = img.resize(width, height, resizetype);

    Ok(newimg.save_with_format(path.as_ref(), image::ImageFormat::Png)?)
}

/// Output is a JSON string for reading within BYOND
///
/// Erroring at any point will produce an empty string
fn read_states(path: &str) -> Result<String> {
    let file = File::open(path).map(BufReader::new)?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info().map_err(|_| Error::InvalidPngData)?;
    let info = reader.info();
    let mut states = Vec::<String>::new();
    for ztxt in &info.compressed_latin1_text {
        let text = ztxt.get_text()?;
        text.lines()
            .take_while(|line| !line.contains("# END DMI"))
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("state = \"")
                    .and_then(|line| line.strip_suffix('"'))
            })
            .for_each(|state| {
                states.push(state.to_owned());
            });
    }
    Ok(serde_json::to_string(&states)?)
}
