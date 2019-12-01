mod thumbnailer {

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

pub struct Thumbnailer {
    source: PathBuf,
    destination: PathBuf,
    image: Option<image::DynamicImage>
}

impl Thumbnailer {
    pub fn generate(path: &PathBuf) -> Result<(), &'static str> {
	let thumbnailer = Thumbnailer { source: PathBuf::from(path), destination: PathBuf::new(), image: None};
	Thumbnailer::create_thumbnail(thumbnailer, path, 128)
	    .and_then(Thumbnailer::calculate_destination)
	    .and_then(Thumbnailer::save_thumbnail)	
    }

    pub fn calculate_path_md5(path: &PathBuf) -> String {
	let path_uri = "file://".to_owned() + path.to_str().unwrap();
	let vec = md5::compute(path_uri).to_vec();
	hex::encode(vec)
    }

    fn create_thumbnail(mut thumbnailer: Thumbnailer, path: &PathBuf, size: u32) -> Result<Thumbnailer, &'static str> {
	let image_format = image::ImageFormat::from_path(path).map_err(|_| "Failed to obtain file format")?;
	let file = File::open(path).map_err(|_| "File to open file")?;
	let reader = BufReader::new(file);
	let image = image::load(reader, image_format).map_err(|_| "Failed to load file")?;
	thumbnailer.image = Some(image.thumbnail(size, size));
	Ok(thumbnailer)
    }

    fn calculate_destination(mut thumbnailer: Thumbnailer) -> Result<Thumbnailer, &'static str> {
	let filename = Thumbnailer::calculate_path_md5(&thumbnailer.source) + ".png";
	thumbnailer.destination = Path::new("/home/filippo/.cache/thumbnails/normal").join(filename);
	Ok(thumbnailer)
    }

    fn save_thumbnail(thumbnailer: Thumbnailer) -> Result<(), &'static str> {
	thumbnailer.image.unwrap().save_with_format(thumbnailer.destination.to_str().unwrap(), image::ImageFormat::PNG)
	    .map_err(|_e| return "Failed to save thumbnail")
    } 
}

}

use docopt::Docopt;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thumbnailer::Thumbnailer;

const USAGE: &'static str = "
Thumbnailer.

Usage:
  thumbnailer [-v] [-r] [--workers=<wk>] <directory>
  thumbnailer (-h | --help)
  thumbnailer --version

Options:
  -h --help        Show this screen.
  --version        Show version.
  -v --verbose     Verbose output
  -r --recursive   Recursive scan.
  -w --workers     Sets the number of workers [default: 1].
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_directory: String,
    flag_verbose: bool,
    flag_recursive: bool,
    flag_workers: usize,
}


fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    println!("{:?}", args);

    if args.flag_workers < 1 {
	println!("The number of workers must be >= 1");
	return;
    }

    let path = Path::new(args.arg_directory.as_str());
    if !path.exists() || !path.is_dir() {
        println!("Directory {} does not exists", args.arg_directory);
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
                                match Thumbnailer::generate(&path) {
                                    Ok(_) => {
                                        println!("Created thumbnail for {}", path.to_str().unwrap())
                                    }
                                    Err(e) => println!(
                                        "Failed to create thumbnail for {}. Error {}",
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
