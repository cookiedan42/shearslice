//! Encode a vector of f32 values as a png image

use core::f32;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, Write};
#[cfg(feature = "std")]
use std::path::Path;

use image::codecs::{gif::GifEncoder, png::PngEncoder};
use image::{
    Delay, DynamicImage, ExtendedColorType, Frame, ImageBuffer, ImageEncoder, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub enum ColorRamp {
    Viridis,
    Plasma,
    Inferno,
    Magma,
    Turbo,
    Cool,
    Warm,
    Grey,
}

fn apply_color_ramp(value: f32, ramp: &ColorRamp) -> (u8, u8, u8) {
    let clamped_value = value.clamp(0.0, 1.0);

    let color = match ramp {
        ColorRamp::Viridis => colorous::VIRIDIS.eval_continuous(clamped_value as f64),
        ColorRamp::Plasma => colorous::PLASMA.eval_continuous(clamped_value as f64),
        ColorRamp::Inferno => colorous::INFERNO.eval_continuous(clamped_value as f64),
        ColorRamp::Magma => colorous::MAGMA.eval_continuous(clamped_value as f64),
        ColorRamp::Turbo => colorous::TURBO.eval_continuous(clamped_value as f64),
        ColorRamp::Cool => colorous::COOL.eval_continuous(clamped_value as f64),
        ColorRamp::Warm => colorous::WARM.eval_continuous(clamped_value as f64),
        ColorRamp::Grey => colorous::GREYS.eval_continuous(clamped_value as f64),
    };

    let (r, g, b) = color.as_tuple();
    (r as u8, g as u8, b as u8)
}

/// `width` is the fast (inner) axis of `data`'s flat ordering and `height` is the slow
/// (outer) axis, i.e. `data[i]` is at `(x = i % width, y = i / width)`.
pub fn to_buffer(
    data: &[f32],
    color_ramp: &crate::layer::ColorRamp,
    width: u32,
    height: u32,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let mut img = ImageBuffer::new(width, height);

    // scale the data to the range [0.0, 1.0]

    let data = data.iter().map(|v| color_ramp.eval(*v));

    for (i, [r, g, b, _]) in data.enumerate() {
        let x = (i % width as usize) as u32;
        let y = (i / width as usize) as u32;
        img.put_pixel(x, y, Rgb([r, g, b]));
    }

    let d = DynamicImage::ImageRgb8(img);
    let d = d.rotate270();
    d.to_rgb8()
}

pub fn to_png(
    data: &[f32],
    color_ramp: &crate::layer::ColorRamp,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let buffer = to_buffer(data, color_ramp, width, height);
    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            buffer.as_raw(),
            buffer.width(),
            buffer.height(),
            ExtendedColorType::Rgb8,
        )
        .unwrap();
    png_bytes
}

/// Encode a vector of png images as a gif, shifting up through the frames
#[cfg(feature = "std")]
pub fn to_gif<P: AsRef<Path>>(
    buffers: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>,
    path: P,
) -> io::Result<()> {
    let file = File::create(path)?;
    let delay = Delay::from_numer_denom_ms(10, 1);

    let mut enc = GifEncoder::new(file);
    let _ = enc.set_repeat(image::codecs::gif::Repeat::Infinite);

    // Convert each ImageBuffer to a Frame and encode
    for img in buffers {
        let img = rgb_to_rgba(img);
        let frame = Frame::from_parts(img, 0, 0, delay);
        let _ = enc.encode_frame(frame);
    }

    Ok(())

    // write out
}

/// Add alpha channel with full opacity to an RgbImage
fn rgb_to_rgba(rgb_img: RgbImage) -> RgbaImage {
    ImageBuffer::from_fn(rgb_img.width(), rgb_img.height(), |x, y| {
        let rgb = rgb_img.get_pixel(x, y);
        Rgba([rgb[0], rgb[1], rgb[2], 255]) // Add full opacity
    })
}

#[cfg(feature = "std")]
pub fn write_png_to_file<P: AsRef<Path>>(
    data: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    path: P,
) -> io::Result<()> {
    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            data.as_raw(),
            data.width(),
            data.height(),
            ExtendedColorType::Rgb8,
        )
        .unwrap();

    let mut file = File::create(path)?;
    file.write_all(&png_bytes)?;
    Ok(())
}

#[cfg(test)]
mod png_tests {
    use super::*;
    use crate::layer::ColorRamp as LayerColorRamp;

    #[test]
    fn to_png_output_matches_requested_non_square_dims() {
        // 3 wide (x_len) x 5 tall (y_len) plane, i.e. flat data at stride y_len=5.
        let x_len = 3usize;
        let y_len = 5usize;
        let data: Vec<f32> = (0..x_len * y_len)
            .map(|i| i as f32 / (x_len * y_len) as f32)
            .collect();
        let ramp: LayerColorRamp = serde_json::from_str(
            r#"[{"color":[0,0,0,255],"position":0.0},{"color":[255,255,255,255],"position":1.0}]"#,
        )
        .unwrap();
        let png_bytes = to_png(&data, &ramp, y_len as u32, x_len as u32);

        let img = image::load_from_memory(&png_bytes).unwrap();
        // rotate270 swaps width/height relative to the pre-rotation buffer
        assert_eq!(img.width(), x_len as u32);
        assert_eq!(img.height(), y_len as u32);
    }
}
