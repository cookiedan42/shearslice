use core::f32;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, Write};
#[cfg(feature = "std")]
use std::path::Path;

#[cfg(feature = "std")]
use image::codecs::{gif::GifEncoder, png::PngEncoder};
#[cfg(feature = "std")]
use image::{
    Delay, ExtendedColorType, Frame, ImageBuffer, ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage,
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

pub fn to_png(data: &[f32], color_ramp: ColorRamp) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let mut img = ImageBuffer::new(1024, 1024);

    // scale the data to the range [0.0, 1.0]
    let max_value = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    let min_value = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));

    let data = data
        .iter()
        .map(|&value| (value - min_value) / (max_value - min_value));

    for (i, value) in data.enumerate() {
        let x = (i % 1024 as usize) as u32;
        let y = (i / 1024 as usize) as u32;

        let (r, g, b) = apply_color_ramp(value, &color_ramp);
        img.put_pixel(x, y, Rgb([r, g, b]));
    }
    img
}

#[cfg(feature = "std")]
pub fn to_gif<P: AsRef<Path>>(
    buffers: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>,
    path: P,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    let delay = Delay::from_numer_denom_ms(10, 1);

    let mut enc = GifEncoder::new(file);
    enc.set_repeat(image::codecs::gif::Repeat::Infinite);

    // Convert each ImageBuffer to a Frame and encode
    for img in buffers {
        let img = rgb_to_rgba(img);
        let frame = Frame::from_parts(img, 0, 0, delay);
        enc.encode_frame(frame);
    }

    Ok(())

    // write out
}

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
        .write_image(data.as_raw(), 1024, 1024, ExtendedColorType::Rgb8)
        .unwrap();

    let mut file = File::create(path)?;
    file.write_all(&png_bytes)?;
    Ok(())
}

#[cfg(feature = "std")]
pub fn to_bmp<P: AsRef<Path>>(data: &[f32], color_ramp: ColorRamp, path: P) {
    let max_val = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    let min_val = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let data = data
        .iter()
        .map(|&value| (value - min_val) / (max_val - min_val));

    let mut img = ImageBuffer::new(1024, 1024);

    for (i, value) in data.enumerate() {
        let x = (i % 1024 as usize) as u32;
        let y = (i / 1024 as usize) as u32;

        let (r, g, b) = apply_color_ramp(value, &color_ramp);

        // if x % 2 == 0 || y % 2 == 0 {
        img.put_pixel(x, y, Rgb([r, g, b]));
        // } else {
        // img.put_pixel(x, y, Rgb([255, 255, 255]));
        // }
    }

    img.save(path).unwrap();
}
