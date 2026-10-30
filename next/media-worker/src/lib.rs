//! The existing one-request JSON protocol, implemented without Python for images and Zstd.
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageReader, Limits, metadata::Orientation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Cursor, Read, Write};

pub const MAX_REQUEST: usize = 24 * 1024 * 1024;
pub const MAX_INPUT: usize = 15 * 1024 * 1024;
pub const MAX_EXPANDED: usize = 16 * 1024 * 1024;
pub const MEMORY_LIMIT: u64 = 512 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug)]
pub struct WorkerError(pub &'static str);
type Result<T> = std::result::Result<T, WorkerError>;

#[derive(Deserialize)]
struct Request {
    op: String,
    data: String,
    #[serde(default = "default_side")]
    max_side: u32,
}
fn default_side() -> u32 {
    1536
}

pub fn process_request(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_REQUEST {
        return Err(WorkerError("RequestTooLarge"));
    }
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| WorkerError("InvalidRequest"))?;
    if request.data.len() > MAX_INPUT.div_ceil(3) * 4 {
        return Err(WorkerError("InputTooLarge"));
    }
    let data = STANDARD
        .decode(&request.data)
        .map_err(|_| WorkerError("InvalidBase64"))?;
    if data.len() > MAX_INPUT {
        return Err(WorkerError("InputTooLarge"));
    }
    match request.op.as_str() {
        "image_resize" => resize(&data, request.max_side),
        "session_zstd" => expand_session(&data),
        _ => Err(WorkerError("UnsupportedOperation")),
    }
}

fn resize(data: &[u8], max_side: u32) -> Result<Value> {
    if !(1..=4096).contains(&max_side) {
        return Err(WorkerError("InvalidImageLimit"));
    }
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| WorkerError("InvalidImage"))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MEMORY_LIMIT);
    limits.max_image_width = Some(MAX_PIXELS as u32);
    limits.max_image_height = Some(MAX_PIXELS as u32);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| WorkerError("InvalidImage"))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(WorkerError("ImageTooLarge"));
    }
    let orientation = decoder
        .orientation()
        .map_err(|_| WorkerError("InvalidImage"))?;
    let icc = decoder
        .icc_profile()
        .map_err(|_| WorkerError("InvalidImage"))?;
    // Validate pixels even when retaining the original file bytes.
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| WorkerError("InvalidImage"))?;
    if width.max(height) <= max_side && orientation == Orientation::NoTransforms {
        return Ok(json!({"ok": true, "changed": false}));
    }
    image.apply_orientation(orientation);
    // Pillow thumbnail never enlarged an image; orientation-only work must not either.
    let image = if image.width().max(image.height()) > max_side {
        image.thumbnail(max_side, max_side)
    } else {
        image
    };
    let mut output = BoundedOutput(Vec::new());
    let mut encoder = image::codecs::png::PngEncoder::new(&mut output);
    if let Some(profile) = icc {
        encoder
            .set_icc_profile(profile)
            .map_err(|_| WorkerError("ImageEncodingFailed"))?;
    }
    encoder
        .write_image(
            image.as_bytes(),
            image.width(),
            image.height(),
            image.color().into(),
        )
        .map_err(|_| WorkerError("ImageEncodingFailed"))?;
    Ok(json!({"ok": true, "changed": true, "data": STANDARD.encode(&output.0)}))
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_INPUT.saturating_sub(self.0.len()) {
            return Err(io::Error::other("output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn expand_session(data: &[u8]) -> Result<Value> {
    // Python's former stream_reader consumed the first frame. Retain that behavior.
    let mut decoder = zstd::stream::read::Decoder::new(Cursor::new(data))
        .map_err(|_| WorkerError("InvalidCompressedSession"))?
        .single_frame();
    decoder
        .window_log_max(28)
        .map_err(|_| WorkerError("InvalidCompressedSession"))?;
    let mut expanded = Vec::new();
    decoder
        .take((MAX_EXPANDED + 1) as u64)
        .read_to_end(&mut expanded)
        .map_err(|_| WorkerError("InvalidCompressedSession"))?;
    if expanded.len() > MAX_EXPANDED {
        return Err(WorkerError("SessionTooLarge"));
    }
    let text = String::from_utf8(expanded).map_err(|_| WorkerError("InvalidSessionText"))?;
    Ok(json!({"ok": true, "text": text}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(op: &str, data: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&json!({"op":op,"data":STANDARD.encode(data)})).unwrap()
    }

    #[test]
    fn empty_valid_session_and_unicode_round_trip() {
        for text in ["", "中文会话\n{\"type\":\"message\"}"] {
            let compressed = zstd::stream::encode_all(text.as_bytes(), 1).unwrap();
            assert_eq!(
                process_request(&request("session_zstd", &compressed)).unwrap()["text"],
                text
            );
        }
    }

    #[test]
    fn decompression_is_bounded_and_rejects_truncation() {
        let compressed =
            zstd::stream::encode_all(Cursor::new(vec![b'x'; MAX_EXPANDED + 1]), 1).unwrap();
        assert_eq!(
            process_request(&request("session_zstd", &compressed))
                .unwrap_err()
                .0,
            "SessionTooLarge"
        );
        let valid = zstd::stream::encode_all(b"hello".as_slice(), 1).unwrap();
        assert!(process_request(&request("session_zstd", &valid[..valid.len() - 1])).is_err());
    }

    #[test]
    fn request_validation_does_not_echo_private_input() {
        assert!(process_request(b"SYNTHETIC_PRIVATE_INPUT").is_err());
        assert!(process_request(br#"{"op":"image_resize","data":"!"}"#).is_err());
        assert_eq!(
            process_request(&request("pdf_text", b"private"))
                .unwrap_err()
                .0,
            "UnsupportedOperation"
        );
    }

    #[test]
    fn png_resize_and_no_resize_contract() {
        let image = image::RgbaImage::from_pixel(80, 40, image::Rgba([100, 50, 25, 128]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut value =
            json!({"op":"image_resize","data":STANDARD.encode(png.get_ref()),"max_side":80});
        assert_eq!(
            process_request(&serde_json::to_vec(&value).unwrap()).unwrap(),
            json!({"ok":true,"changed":false})
        );
        value["max_side"] = json!(20);
        let result = process_request(&serde_json::to_vec(&value).unwrap()).unwrap();
        let bytes = STANDARD.decode(result["data"].as_str().unwrap()).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (20, 10));
        assert_eq!(decoded.to_rgba8().get_pixel(10, 5).0[3], 128);
        value["max_side"] = json!(0);
        assert_eq!(
            process_request(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .0,
            "InvalidImageLimit"
        );
    }
}
