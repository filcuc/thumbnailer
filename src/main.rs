use docopt::Docopt;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use std::fs::File;
use std::io::BufReader;

const USAGE: &'static str = "
Thumbnailer.

Usage:
  thumbnailer [-v] [-r] <directory>
  thumbnailer (-h | --help)
  thumbnailer --version

Options:
  -h --help        Show this screen.
  --version        Show version.
  -v --verbose     Verbose output
  -r --recursive   Recursive scan.
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_directory: String,
    flag_verbose: bool,
    flag_recursive: bool,
}

fn calculate_path_md5(path: &PathBuf) -> String {
    let path_uri = "file://".to_owned() + path.to_str().unwrap();
    let vec = md5::compute(path_uri).to_vec();
    hex::encode(vec)
}

fn create_thumbnail(path: &PathBuf) -> Result<(), &'static str> {
    let image_format = match image::ImageFormat::from_path(path) {
	Ok(f) => f,
	Err(_) => return Err("Failed to obtain file format")
    };

    let file = match File::open(path) {
	Ok(f) => f,
	Err(_) => return Err("File to open file")
    };
    
    let reader = BufReader::new(file);

    let image = match image::load(reader, image_format) {
	Ok(i) => i,
	Err(_) => return Err("Failed to load file")
    };

    let path_md5 = calculate_path_md5(path);

    let thumbnail = image.thumbnail(128, 128);

    let destination = Path::new("/home/filippo/.cache/thumbnails/normal").join(path_md5 + ".png");

    match thumbnail.save_with_format(destination.to_str().unwrap(), image::ImageFormat::PNG) {
	Ok(_) => Ok(()),
	Err(e) => { println!("{}", e); Err("Failed to save thumbnail {}") }
    }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    println!("{:?}", args);

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
                            let extension = path.extension().unwrap_or_default().to_str().unwrap().to_lowercase();
			    println!("Found a file with extension {}", extension);
			    if extension == "jpg" || extension == "jpeg" || extension == "png" {
				match create_thumbnail(&path) {
				    Ok(_) => println!("Created thumbnail for {}", path.to_str().unwrap()),
				    Err(e) => println!("Failed to create thumbnail for {}. Error {}", path.to_str().unwrap(), e)
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
    use crate::calculate_path_md5;
    use std::path::Path;
    use std::fs;

    #[test]
    fn it_works() {
	let path = Path::new("/home/jens/photos/me.png").to_owned();
        assert_eq!(calculate_path_md5(&path), "c6ee772d9e49320e97ec29a7eb5b1697".to_owned());
    }
}
