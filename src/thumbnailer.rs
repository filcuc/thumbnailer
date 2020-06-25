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
use log::{debug, error};
use percent_encoding::{AsciiSet, CONTROLS};
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

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
    temp_path: PathBuf,
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
            temp_path: PathBuf::new(),
            destination_path: PathBuf::new(),
            filename: String::new(),
            image: None,
            thumbnail: None,
            thumbnail_size: image_size,
            use_full_path_for_md5: use_full_path_for_md5,
        };
        Thumbnailer::create_thumbnail_in_memory(thumbnailer)
            .and_then(Thumbnailer::calculate_filename)
            .and_then(Thumbnailer::calculate_temporary_destination)
            .and_then(Thumbnailer::calculate_destination)
            .and_then(Thumbnailer::save_thumbnail_to_temp)
            .and_then(Thumbnailer::update_metadata)
            .and_then(Thumbnailer::move_thumbnail_to_destination)
    }

    fn calculate_path_uri(use_full_path_for_md5: bool, path: &PathBuf) -> String {
        const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
        const PATH_SET: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');
        const USER_INFO_SET: &AsciiSet = &PATH_SET
            .add(b'/')
            .add(b':')
            .add(b';')
            .add(b'=')
            .add(b'@')
            .add(b'[')
            .add(b'\\')
            .add(b']')
            .add(b'^')
            .add(b'|');

        assert!(path.is_absolute());

        if use_full_path_for_md5 {
            let mut encoded = String::new();

            for t in path.iter() {
                if t == OsStr::new(&std::path::MAIN_SEPARATOR.to_string()) {
                    continue;
                } else {
                    encoded.push(std::path::MAIN_SEPARATOR);
                    encoded +=
                        &percent_encoding::utf8_percent_encode(t.to_str().unwrap(), USER_INFO_SET)
                            .to_string();
                }
            }
            format!("file://{}", encoded)
        } else {
            percent_encoding::utf8_percent_encode(
                path.file_name().unwrap().to_str().unwrap(),
                USER_INFO_SET,
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

    fn calculate_temporary_destination(
        mut thumbnailer: Thumbnailer,
    ) -> Result<Thumbnailer, String> {
        thumbnailer.temp_path = std::env::temp_dir().join(&thumbnailer.filename);
        if thumbnailer.temp_path.exists() {
            if let Err(_) = std::fs::remove_file(&thumbnailer.temp_path) {
                return Err("Failed to remove temporary".to_owned());
            }
        }

        Ok(thumbnailer)
    }

    fn calculate_destination(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        thumbnailer.destination_path = thumbnailer
            .cache_path
            .join(thumbnailer.thumbnail_size.name())
            .join(&thumbnailer.filename);

        if thumbnailer.destination_path.exists() {
            if let Err(_) = std::fs::remove_file(&thumbnailer.destination_path) {
                return Err("Failed to remove existing thumbnail in destionation dir".to_owned());
            }
        }

        debug!(
            "Saving thumb in {}",
            thumbnailer.destination_path.to_str().unwrap()
        );
        Ok(thumbnailer)
    }

    fn save_thumbnail_to_temp(thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        &thumbnailer
            .thumbnail
            .as_ref()
            .unwrap()
            .save_with_format(
                thumbnailer.temp_path.to_str().unwrap(),
                image::ImageFormat::PNG,
            )
            .map_err(|e| {
                error!("{}", e);
                "Failed to save thumbnail".to_owned()
            });
        Ok(thumbnailer)
    }

    fn update_metadata(thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        let mut chunks = {
            let mut input = std::fs::File::open(&thumbnailer.temp_path)
                .map_err(|_e| "Failed to open thumbnailer in temporary dir".to_owned())?;
            crate::png::Png::decode(&mut input).map_err(|_e| "Failed decoding chunks".to_owned())?
        };

        let uri_raw = Thumbnailer::calculate_path_uri(
            thumbnailer.use_full_path_for_md5,
            &thumbnailer.source_path,
        );
        let uri = crate::png::Chunk::new_text("Thumb::URI", uri_raw).unwrap();
        chunks.insert(1, uri);

        let metadata = std::fs::metadata(&thumbnailer.source_path).unwrap();

        let mtime_raw = metadata.modified().unwrap();
        let mtime_raw = mtime_raw.duration_since(std::time::UNIX_EPOCH).unwrap();
        let mtime_raw = mtime_raw.as_secs();
        let mtime = crate::png::Chunk::new_text("Thumb::MTime", mtime_raw.to_string()).unwrap();
        chunks.insert(1, mtime);

        let size_raw = metadata.len();
        let size = crate::png::Chunk::new_text("Thumb::Size", size_raw.to_string()).unwrap();
        chunks.insert(1, size);

        let width = crate::png::Chunk::new_text(
            "Thumb::Image::Width",
            thumbnailer.image.as_ref().unwrap().width().to_string(),
        )
        .unwrap();
        chunks.insert(1, width);

        let height = crate::png::Chunk::new_text(
            "Thumb::Image::Height",
            thumbnailer.image.as_ref().unwrap().height().to_string(),
        )
        .unwrap();
        chunks.insert(1, height);

        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .open(&thumbnailer.temp_path)
            .map_err(|_e| "Failed to open thumbnailer in temporary dir".to_owned())?;
        crate::png::Png::encode(&mut output, &chunks).map_err(|e| {
            error!("Error {:?}", e);
            "Failed to encode chunks to temporary file"
        })?;

        Ok(thumbnailer)
    }

    fn move_thumbnail_to_destination(thumbnailer: Thumbnailer) -> Result<(), String> {
        let r = std::process::Command::new("mv")
            .arg(&thumbnailer.temp_path.to_str().unwrap())
            .arg(&thumbnailer.destination_path.to_str().unwrap())
            .status();
        if r.is_ok() && r.unwrap().success() {
            Ok(())
        } else {
            Err(format!(
                "Failed to move from {} to {}",
                &thumbnailer.temp_path.to_str().unwrap(),
                &thumbnailer.destination_path.to_str().unwrap()
            ))
        }
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
