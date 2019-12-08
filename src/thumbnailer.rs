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
    destination_path: PathBuf,
    image: Option<image::DynamicImage>,
    image_size: ThumbSize,
}

impl Thumbnailer {
    pub fn generate(source_path: PathBuf, cache_path: PathBuf, image_size: ThumbSize) -> Result<(), String> {
        let thumbnailer = Thumbnailer {
            source_path,
            cache_path,
            destination_path: PathBuf::new(),
            image: None,
            image_size,
        };
        Thumbnailer::create_thumbnail(thumbnailer)
            .and_then(Thumbnailer::calculate_destination)
            .and_then(Thumbnailer::save_thumbnail)
    }

    pub fn calculate_path_md5(path: &PathBuf) -> String {
        let path_uri = "file://".to_owned() + path.to_str().unwrap();
        let vec = md5::compute(path_uri).to_vec();
        hex::encode(vec)
    }

    fn create_thumbnail(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        let image_format = image::ImageFormat::from_path(&thumbnailer.source_path)
            .map_err(|_| "Failed to obtain file format".to_owned())?;
        let file = File::open(&thumbnailer.source_path).map_err(|_| "File to open file".to_owned())?;
        let reader = BufReader::new(file);
        let image =
            image::load(reader, image_format).map_err(|_| "Failed to load file".to_owned())?;
        thumbnailer.image = Some(image.thumbnail(thumbnailer.image_size.size(), thumbnailer.image_size.size()));
        Ok(thumbnailer)
    }

    fn calculate_destination(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, String> {
        let filename = Thumbnailer::calculate_path_md5(&thumbnailer.source_path) + ".png";
        thumbnailer.destination_path = thumbnailer.cache_path.join(thumbnailer.image_size.name()).join(filename);
        println!(
            "Saving thumb in {}",
            thumbnailer.destination_path.to_str().unwrap()
        );
        Ok(thumbnailer)
    }

    fn save_thumbnail(thumbnailer: Thumbnailer) -> Result<(), String> {
        println!("{}", thumbnailer.destination_path.to_str().unwrap());
        thumbnailer
            .image
            .unwrap()
            .save_with_format(
                thumbnailer.destination_path.to_str().unwrap(),
                image::ImageFormat::PNG,
            )
            .map_err(|e| { println!("{}", e ); "Failed to save thumbnail".to_owned()})
    }
}

#[cfg(test)]
mod tests {
    use crate::thumbnailer::Thumbnailer;
    use std::path::Path;

    #[test]
    fn it_works() {
        let path = Path::new("/home/jens/photos/me.png").to_owned();
        assert_eq!(
            Thumbnailer::calculate_path_md5(&path),
            "c6ee772d9e49320e97ec29a7eb5b1697".to_owned()
        );
    }
}