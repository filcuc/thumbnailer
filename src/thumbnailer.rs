/**
    This file is part of Thumbnailer.

    Thumbnailer is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License.

    Thumbnailer is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with Thumbnailer.  If not, see <http://www.gnu.org/licenses/>.
*/
use image::GenericImageView;
use log::debug;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

const PNG_TEXT_KIND: [u8; 4] = ['t' as u8, 'E' as u8, 'X' as u8, 't' as u8];

pub fn text_chunk<S: Into<String>>(keyword: &str, text: S) -> Result<Vec<u8>, ()> {
    let text = text.into();

    if keyword.is_empty() || keyword.len() > 79 || keyword.contains('\0') {
        return Err(());
    }

    if text.contains('\0') {
        return Err(());
    }

    let text = text.replace("\r\n", "\n");

    if text.is_empty() {
        return Err(());
    }
    let data = {
        let mut r = vec![];
        r.extend_from_slice(keyword.as_bytes());
        r.push(0);
        r.extend_from_slice(text.as_bytes());
        r
    };

    Ok(data)
}

#[derive(Copy, Clone)]
pub enum ThumbSize {
    Normal,
    Large,
}

impl ThumbSize {
    fn size(&self) -> u32 {
        match self {
            ThumbSize::Normal => 128,
            ThumbSize::Large => 256,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThumbSize::Normal => "normal",
            ThumbSize::Large => "large",
        }
    }
}

pub struct Thumbnailer {
    source_path: PathBuf,
    cache_path: PathBuf,
    destination_path: PathBuf,
    image: Option<image::DynamicImage>,
    thumbnail: Option<image::DynamicImage>,
    thumbnail_size: ThumbSize,
    pub filename: String,
    use_full_path_for_md5: bool,
}

impl Thumbnailer {
    pub fn generate(
        source_path: PathBuf,
        cache_path: PathBuf,
        image_size: ThumbSize,
        use_full_path_for_md5: bool,
    ) -> Result<(), String> {
        let source_path = source_path
            .canonicalize()
            .map_err(|_e| "Cannot normalize input path")?;
        let thumbnailer = Thumbnailer {
            source_path,
            cache_path,
            destination_path: PathBuf::new(),
            filename: String::new(),
            image: None,
            thumbnail: None,
            thumbnail_size: image_size,
            use_full_path_for_md5: use_full_path_for_md5,
        };
        Thumbnailer::create_thumbnail_in_memory(thumbnailer)
            .and_then(Thumbnailer::calculate_filename)
            .and_then(Thumbnailer::calculate_destination)
            .and_then(Thumbnailer::save_thumbnail_to_temp)
            .and_then(Thumbnailer::move_thumbnail_to_destination)
    }

    fn calculate_path_uri(use_full_path_for_md5: bool, path: &PathBuf) -> String {
        /// The characters that need to be escaped to minimally obtain the `pchar` production of
        /// RFC3986
        const PCHAR: AsciiSet = NON_ALPHANUMERIC
            // Allow the (still missing) non-alphanumeric parts from `unreserved`
            .remove(b'-')
            .remove(b'.')
            .remove(b'_')
            .remove(b'~')
            // Nothing to do for `pct-encoded`, these are the remaining characters.
            // Allow `sub-delims`
            .remove(b'!')
            .remove(b'$')
            .remove(b'&')
            .remove(b'\'')
            .remove(b'(')
            .remove(b')')
            .remove(b'*')
            .remove(b'+')
            .remove(b',')
            .remove(b';')
            .remove(b'=')
            // Allow the explicitly allowed characters
            .remove(b':')
            .remove(b'@')
            ;
        const PATH_TO_FILEURI: AsciiSet = PCHAR.remove(b'/');

        assert!(path.is_absolute());

        if use_full_path_for_md5 {
            let mut encoded = String::new();

            for t in path.iter() {
                if t == OsStr::new(&std::path::MAIN_SEPARATOR.to_string()) {
                    continue;
                } else {
                    encoded.push(std::path::MAIN_SEPARATOR);
                    encoded +=
                        &percent_encoding::utf8_percent_encode(t.to_str().unwrap(), &PATH_TO_FILEURI)
                            .to_string();
                }
            }
            format!("file://{}", encoded)
        } else {
            percent_encoding::utf8_percent_encode(
                path.file_name().unwrap().to_str().unwrap(),
                // Could just as well pick pchar, and while arguably it'd be more correct, no need
                // to lug around two tables as practically the path is guaranteed not to contain a
                // slash.
                &PATH_TO_FILEURI,
            )
            .to_string()
        }
    }

    pub fn calculate_path_md5(use_full_path_for_md5: bool, path: &PathBuf) -> String {
        let uri = Thumbnailer::calculate_path_uri(use_full_path_for_md5, &path);
        let vec = md5::compute(uri).to_vec();
        hex::encode(vec)
    }

    fn create_thumbnail_in_memory(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        let image_format = image::ImageFormat::from_path(&thumbnailer.source_path)
            .map_err(|_| "Failed to obtain file format".to_owned())?;
        let file =
            File::open(&thumbnailer.source_path).map_err(|_| "File to open file".to_owned())?;
        let reader = BufReader::new(file);
        let image =
            image::load(reader, image_format).map_err(|_| "Failed to load file".to_owned())?;
        let thumbnail = image.thumbnail(
            thumbnailer.thumbnail_size.size(),
            thumbnailer.thumbnail_size.size(),
        );
        if thumbnail.width() == 0 || thumbnail.height() == 0 {
            return Err(format!(
                "Thumbnail width or height < 0 for image {}",
                thumbnailer.source_path.to_str().unwrap()
            ));
        }
        thumbnailer.thumbnail = Some(thumbnail);
        thumbnailer.image = Some(image);
        Ok(thumbnailer)
    }

    fn calculate_filename(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        thumbnailer.filename = Thumbnailer::calculate_path_md5(
            thumbnailer.use_full_path_for_md5,
            &thumbnailer.source_path,
        ) + ".png";
        Ok(thumbnailer)
    }

    fn calculate_destination(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        thumbnailer.destination_path = thumbnailer
            .cache_path
            .join(thumbnailer.thumbnail_size.name())
            .join(&thumbnailer.filename);

        debug!(
            "Saving thumb in {}",
            thumbnailer.destination_path.to_str().unwrap()
        );
        Ok(thumbnailer)
    }

    fn save_thumbnail_to_temp(thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        let temp_path = format!("{}.tmp", thumbnailer.destination_path.to_str().unwrap());
        let output = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&temp_path)
            .map_err(|e| format!("Failed to open thumbnailer in temporary dir: {}", e))?;

        let thumbnail = thumbnailer.thumbnail.as_ref().unwrap();
        let (ct, bits) = match thumbnail.color() {
            image::ColorType::L8 => (png::ColorType::Grayscale, png::BitDepth::Eight),
            image::ColorType::L16 => (png::ColorType::Grayscale, png::BitDepth::Sixteen),
            image::ColorType::La8 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight),
            image::ColorType::La16 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen),
            image::ColorType::Rgb8 => (png::ColorType::RGB, png::BitDepth::Eight),
            image::ColorType::Rgb16 => (png::ColorType::RGB, png::BitDepth::Sixteen),
            image::ColorType::Rgba8 => (png::ColorType::RGBA, png::BitDepth::Eight),
            image::ColorType::Rgba16 => (png::ColorType::RGBA, png::BitDepth::Sixteen),
            _ => return Err("unsupported format".to_string()),
        };
        let mut encoder = png::Encoder::new(output, thumbnail.width(), thumbnail.height());
        encoder.set_color(ct);
        encoder.set_depth(bits);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("Error writing PNG header: {}", e))?;

        let uri_raw = Thumbnailer::calculate_path_uri(
            thumbnailer.use_full_path_for_md5,
            &thumbnailer.source_path,
        );
        writer
            .write_chunk(PNG_TEXT_KIND, &text_chunk("Thumb::URI", uri_raw).unwrap())
            .map_err(|e| format!("Error writing PNG chunk: {}", e))?;

        let metadata = std::fs::metadata(&thumbnailer.source_path).unwrap();
        let mtime_raw = metadata.modified().unwrap();
        let mtime_raw = mtime_raw.duration_since(std::time::UNIX_EPOCH).unwrap();
        let mtime_raw = mtime_raw.as_secs();
        writer
            .write_chunk(
                PNG_TEXT_KIND,
                &text_chunk("Thumb::MTime", mtime_raw.to_string()).unwrap(),
            )
            .map_err(|e| format!("Error writing PNG chunk: {}", e))?;

        let size_raw = metadata.len();
        writer
            .write_chunk(
                PNG_TEXT_KIND,
                &text_chunk("Thumb::Size", size_raw.to_string()).unwrap(),
            )
            .map_err(|e| format!("Error writing PNG chunk: {}", e))?;
        writer
            .write_chunk(
                PNG_TEXT_KIND,
                &text_chunk(
                    "Thumb::Image::Width",
                    thumbnailer.image.as_ref().unwrap().width().to_string(),
                )
                .unwrap(),
            )
            .map_err(|e| format!("Error writing PNG chunk: {}", e))?;
        writer
            .write_chunk(
                PNG_TEXT_KIND,
                &text_chunk(
                    "Thumb::Image::Height",
                    thumbnailer.image.as_ref().unwrap().height().to_string(),
                )
                .unwrap(),
            )
            .map_err(|e| format!("Error writing PNG chunk: {}", e))?;
        writer
            .write_image_data(&thumbnail.to_bytes())
            .map_err(|e| format!("Error writing PNG image data: {}", e))?;
        Ok(thumbnailer)
    }

    fn move_thumbnail_to_destination(thumbnailer: Thumbnailer) -> Result<(), String> {
        let dst = &thumbnailer.destination_path.to_str().unwrap();
        let src = &format!("{}.tmp", &dst);
        std::fs::rename(src, dst)
            .map_err(|e| format!("Failed to move from {} to {}: {}", src, dst, e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::thumbnailer::{ThumbSize, Thumbnailer};
    use std::path::{Path, PathBuf};

    #[test]
    fn test_calculate_path_md5() {
        let path = Path::new("/home/jens/photos/me.png").to_owned();
        assert_eq!(
            Thumbnailer::calculate_path_md5(true, &path),
            "c6ee772d9e49320e97ec29a7eb5b1697".to_owned()
        );
        assert_eq!(
            Thumbnailer::calculate_path_md5(false, &path),
            "7accaff1d29c5d074218919d4150d1e5".to_owned()
        );
    }

    #[test]
    fn test_new() {
        let input_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("test_resources")
            .join("image.png");
        std::fs::create_dir_all("/tmp/thumbnailer/normal").unwrap();
        std::fs::create_dir_all("/tmp/thumbnailer/large").unwrap();
        Thumbnailer::generate(
            input_path.clone(),
            PathBuf::from("/tmp/thumbnailer"),
            ThumbSize::Normal,
            true,
        )
        .unwrap();
        Thumbnailer::generate(
            input_path.clone(),
            PathBuf::from("/tmp/thumbnailer"),
            ThumbSize::Large,
            true,
        )
        .unwrap();
        Thumbnailer::generate(
            input_path.clone(),
            PathBuf::from("/tmp/thumbnailer"),
            ThumbSize::Normal,
            false,
        )
        .unwrap();
        Thumbnailer::generate(
            input_path.clone(),
            PathBuf::from("/tmp/thumbnailer"),
            ThumbSize::Large,
            false,
        )
        .unwrap();
    }
}
