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

use docopt::Docopt;
use env_logger::Env;
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const USAGE: &'static str = "
Thumbnailer.

Usage:
  thumbnailer [-v] [-r] [--jobs=<num>] (-n|-l|-n -l) (--output=<dir>|-x) <directory>
  thumbnailer [-v] [--jobs=<num>] (-n|-l|-n -l) -s <directory>
  thumbnailer (-h | --help)
  thumbnailer (-v | --verbose)

Options:
  -h --help           Show this screen.
  --version           Show version.
  -v --verbose        Verbose output.
  -d --debug          Debug output.
  -r --recursive      Recursive scan.
  -n --normal         Generate normal thumbs.
  -l --large          Generate large thumbs.
  -j --jobs=<num>     Number of parallel jobs [default: 1]
  -o --output=<dir>   Output to custom directory
  -x --xdg            Output to XDG directory
  -s --shared         Output to shared repository directory
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_directory: String,
    flag_verbose: bool,
    flag_debug: bool,
    flag_recursive: bool,
    flag_normal: bool,
    flag_large: bool,
    flag_workers: Option<u32>,
    flag_output: Option<String>,
    flag_xdg: bool,
    flag_shared: bool,
    flag_jobs: Option<i32>,
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
    if args.flag_output.is_none() && !args.flag_xdg && !args.flag_shared {
        Err("No output or xdg or shared repository argument".to_owned())
    } else if let Some(path) = &args.flag_output {
        Ok(PathBuf::from(path))
    } else if args.flag_shared {
        Ok(PathBuf::from(&args.arg_directory).join(".sh_thumbnails"))
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

struct Work {
    path: PathBuf,
    size: ThumbSize,
    destination: PathBuf,
    use_full_path_for_md5: bool,
}

fn main() {
    // Handle SIGTERM
    ctrlc::set_handler(move || {
        warn!("Handling CTRL-C");
        std::process::exit(1);
    })
    .expect("Error setting Ctrl-C handler");

    // Collect arguments
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .and_then(|mut a: Args| {
            a.arg_directory = shellexpand::full(&a.arg_directory).unwrap().to_string();
            Ok(a)
        })
        .unwrap_or_else(|e| e.exit());

    let level = {
        if args.flag_debug {
            "debug"
        } else if args.flag_verbose {
            "info"
        } else {
            "warn"
        }
    };
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

    debug!("Output directory is {}", destination.to_str().unwrap());

    // Create directories
    for size in args.sizes() {
        let size_directory = destination.join(size.name());
        if !size_directory.exists() {
            debug!(
                "Cache directory {} does not exists",
                size_directory.to_str().unwrap()
            );
            if let Err(_e) = std::fs::create_dir_all(&size_directory) {
                error!(
                    "Failed to create directory {}",
                    size_directory.to_str().unwrap()
                );
                return;
            } else {
                debug!("Created directory {}", size_directory.to_str().unwrap());
            }
        } else {
            debug!(
                "Cache directory {} already exists",
                size_directory.to_str().unwrap()
            );
        }
    }

    // Prepare threads
    let jobs = args.flag_jobs.unwrap_or(1) as usize;
    debug!("Initializing {} workers", jobs);

    // Prepare walk iterator
    let mut walk = walkdir::WalkDir::new(path).min_depth(1);
    if !args.flag_recursive {
        walk = walk.max_depth(1);
    }

    let (sender, receiver) = crossbeam::channel::bounded(jobs * 2);
    let mut workers = Vec::new();
    for _ in 0..jobs {
        let r = receiver.clone();
        workers.push(std::thread::spawn(move || loop {
            let work: Work = match r.recv() {
                Ok(v) => v,
                Err(_) => break,
            };
            match Thumbnailer::generate(
                work.path.clone(),
                work.destination.clone(),
                work.size,
                work.use_full_path_for_md5,
            ) {
                Ok(_) => info!(
                    "Created {} thumbnail for {}",
                    work.size.name(),
                    work.path.canonicalize().unwrap().to_str().unwrap()
                ),
                Err(e) => error!(
                    "Failed to create {} thumbnail for {}. Error {}",
                    work.size.name(),
                    work.path.to_str().unwrap(),
                    e
                ),
            }
        }))
    }

    // Walk filesystem
    walk.into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_image(e))
        .map(|e| e.path().to_path_buf())
        .for_each(|p| {
            for size in args.sizes() {
                sender
                    .send(Work {
                        path: p.clone(),
                        destination: destination.clone(),
                        use_full_path_for_md5: !args.flag_shared,
                        size,
                    })
                    .unwrap()
            }
        });
    drop(sender);
    for w in workers {
        w.join().unwrap();
    }
}
