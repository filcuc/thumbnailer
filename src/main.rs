mod thumbnailer {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;

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

        fn dir_name(&self) -> &'static str {
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
            thumbnailer.destination_path = thumbnailer.cache_path.join(thumbnailer.image_size.dir_name()).join(filename);
            println!(
                "Saving thumb in {}",
                thumbnailer.destination_path.to_str().unwrap()
            );
            Ok(thumbnailer)
        }

        fn save_thumbnail(thumbnailer: Thumbnailer) -> Result<(), String> {
            thumbnailer
                .image
                .unwrap()
                .save_with_format(
                    thumbnailer.destination_path.to_str().unwrap(),
                    image::ImageFormat::PNG,
                )
                .map_err(|_e| return "Failed to save thumbnail".to_owned())
        }
    }
}

use docopt::Docopt;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thumbnailer::{ThumbSize, Thumbnailer};

const USAGE: &'static str = "
Thumbnailer.

Usage:
  thumbnailer [--verbose] [--recursive] (--small|--large) (--output=<dir>|--xdg) <directory>
  thumbnailer (-h | --help)
  thumbnailer --version

Options:
  -h --help           Show this screen.
  --version           Show version.
  -v --verbose        Verbose output.
  -r --recursive      Recursive scan.
  -s --small          Generate small thumbs.
  -l --large          Generate large thumbs.
  -o --output=<dir>   Custom Output directory
  -x --xdg            XDG directory
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_directory: String,
    flag_verbose: bool,
    flag_recursive: bool,
    flag_small: bool,
    flag_large: bool,
    flag_workers: Option<u32>,
    flag_output: Option<String>,
    flag_xdg: bool,
}

fn get_cache_destination(args: &Args) -> Result<PathBuf, String> {
    if args.flag_output.is_none() && !args.flag_xdg {
        Err("No output nore xdg arguments".to_owned())
    } else if let Some(path) = &args.flag_output {
        Ok(PathBuf::from(path))
    } else if let Ok(path) = std::env::var("XDG_CACHE_HOME") {
        Ok(PathBuf::from(path).join("thumbnails"))
    } else if let Ok(path) = std::env::var("HOME") {
        Ok(PathBuf::from(path).join(".cache").join("thumbnails"))
    } else {
        Err("Failed to obtain XDG_CACHE_HOME or HOME".to_owned())
    }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .and_then(|mut a: Args| {
            a.arg_directory = shellexpand::full(&a.arg_directory).unwrap().to_string();
            Ok(a)
        })
        .unwrap_or_else(|e| e.exit());

    // Check input directory
    let path = Path::new(args.arg_directory.as_str());
    if !path.exists() || !path.is_dir() {
        println!("Directory {} does not exists", args.arg_directory);
        return;
    }

    // Check destination directory
    let destination = match get_cache_destination(&args) {
        Ok(p) => p,
        Err(msg) => {
            println!("{}", msg);
            return;
        }
    };

    if !destination.exists() || !destination.is_dir() {
        println!("Cache directory {} does not exists", destination.to_str().unwrap());
        return;
    }

    let mut queue: Vec<PathBuf> = Vec::new();
    queue.push(path.to_owned());

    while !queue.is_empty() {
        let dir: PathBuf = queue.pop().unwrap();

        println!("Processing directory {}", dir.to_str().unwrap());

        if let Ok(dirs) = dir.read_dir() {
            for entry in dirs {
                if let Ok(entry) = entry {
                    if let Ok(file_type) = entry.file_type() {
                        if args.flag_recursive && file_type.is_dir() {
                            queue.push(entry.path());
                        }
                        if file_type.is_file() {
                            let path: PathBuf = entry.path();
                            let extension = path
                                .extension()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap()
                                .to_lowercase();
                            println!("Found a file with extension {}", extension);
                            if extension == "jpg" || extension == "jpeg" || extension == "png" {
                                if args.flag_small {
                                    match Thumbnailer::generate(path.clone(), destination.clone(), ThumbSize::Normal) {
                                        Ok(_) => {
                                            println!("Created small thumbnail for {}", path.to_str().unwrap())
                                        }
                                        Err(e) => println!(
                                            "Failed to create small thumbnail for {}. Error {}",
                                            path.to_str().unwrap(),
                                            e
                                        ),
                                    }
                                }
                                if args.flag_large {
                                    match Thumbnailer::generate(path.clone(), destination.clone(), ThumbSize::Large) {
                                        Ok(_) => {
                                            println!("Created large thumbnail for {}", path.to_str().unwrap())
                                        }
                                        Err(e) => println!(
                                            "Failed to create large thumbnail for {}. Error {}",
                                            path.to_str().unwrap(),
                                            e
                                        ),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
