use core::f32;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use image::Rgb;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, codecs::png::PngEncoder};

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

pub fn to_png(data: &[f32], color_ramp: ColorRamp) -> Vec<u8> {
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

    // Encode to PNG using the correct API
    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(img.as_raw(), 1024, 1024, ExtendedColorType::Rgb8)
        .unwrap();

    png_bytes
}

pub fn write_png_to_file<P: AsRef<Path>>(png_data: &[u8], path: P) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(png_data)?;
    Ok(())
}
