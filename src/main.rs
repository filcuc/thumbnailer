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
mod thumbnailer;
use crate::thumbnailer::{ThumbSize, Thumbnailer};

mod png;
mod worker;

use docopt::Docopt;
use env_logger::Env;
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const USAGE: &'static str = "
Thumbnailer.

Usage:
  thumbnailer [--verbose] [--recursive] (--normal|--large) (--output=<dir>|--xdg) <directory>
  thumbnailer (-h | --help)
  thumbnailer --version

Options:
  -h --help           Show this screen.
  --version           Show version.
  -v --verbose        Verbose output.
  -r --recursive      Recursive scan.
  -n --normal         Generate normal thumbs.
  -l --large          Generate large thumbs.
  -o --output=<dir>   Custom Output directory
  -x --xdg            XDG directory
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_directory: String,
    flag_verbose: bool,
    flag_recursive: bool,
    flag_normal: bool,
    flag_large: bool,
    flag_workers: Option<u32>,
    flag_output: Option<String>,
    flag_xdg: bool,
}

impl Args {
    fn sizes(&self) -> Vec<ThumbSize> {
        let mut result = Vec::new();
        if self.flag_normal {
            result.push(ThumbSize::Normal)
        }
        if self.flag_large {
            result.push(ThumbSize::Large)
        }
        result
    }
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

fn is_image(entry: &walkdir::DirEntry) -> bool {
    let extension = match entry.path().extension() {
        Some(e) => e,
        _ => return false,
    };
    let extensions = extension.to_str().unwrap().to_lowercase();
    extensions == "jpg" || extensions == "jpeg" || extensions == "png"
}

fn generate_thumbnail(path: PathBuf, sizes: Vec<ThumbSize>, destination: &PathBuf) {
    for size in sizes {
        match Thumbnailer::generate(path.clone(), destination.clone(), size) {
            Ok(_) => info!(
                "Created {} thumbnail for {}",
                size.name(),
                path.canonicalize().unwrap().to_str().unwrap()
            ),
            Err(e) => error!(
                "Failed to create {} thumbnail for {}. Error {}",
                size.name(),
                path.to_str().unwrap(),
                e
            ),
        }
    }
}

fn main() {
    // Collect arguments
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .and_then(|mut a: Args| {
            a.arg_directory = shellexpand::full(&a.arg_directory).unwrap().to_string();
            Ok(a)
        })
        .unwrap_or_else(|e| e.exit());

    let level = if args.flag_verbose { "debug" } else { "info" };
    env_logger::from_env(Env::default().default_filter_or(level)).init();

    // Check input directory
    let path = Path::new(args.arg_directory.as_str());
    if !path.exists() || !path.is_dir() {
        error!("Directory {} does not exists", args.arg_directory);
        return;
    }

    // Check input directory existence
    if !path.exists() || !path.is_dir() {
        error!("Input directory {} does not exists", path.to_str().unwrap());
        return;
    }

    // Check destination directory
    let destination = match get_cache_destination(&args) {
        Ok(p) => p,
        Err(msg) => {
            error!("{}", msg);
            return;
        }
    };

    // Create directories
    for size in args.sizes() {
        let size_directory = destination.join(size.name());
        if !size_directory.exists() {
            debug!(
                "Cache directory {} does not exists",
                size_directory.to_str().unwrap()
            );
            if let Err(e) = std::fs::create_dir_all(&size_directory) {
                error!(
                    "Failed to create directory {}",
                    size_directory.to_str().unwrap()
                );
                return;
            } else {
                debug!("Created directory {}", size_directory.to_str().unwrap());
            }
        }
    }

    // Prepare threads
    let mut w = worker::Worker::new(4);

    // Prepare walk iterator
    let mut walk = walkdir::WalkDir::new(path).min_depth(1);
    if args.flag_recursive {
        walk = walk.max_depth(1);
    }

    // Walk filesystem
    walk.into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_image(e))
        .map(|e| e.path().to_path_buf())
        .for_each(|p| w.push(p.clone(), args.sizes(), destination.clone()));
}
