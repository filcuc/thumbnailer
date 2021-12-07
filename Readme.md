# Thumbnailer

[![Build Status](https://travis-ci.org/filcuc/thumbnailer.svg?branch=master)](https://travis-ci.org/filcuc/thumbnailer)
[![codecov](https://codecov.io/gh/filcuc/thumbnailer/branch/master/graph/badge.svg)](https://codecov.io/gh/filcuc/thumbnailer)



Thumbnailer is an app for creating thumbnails following the 
[XDG standard version 0.8.0](https://specifications.freedesktop.org/thumbnail-spec/thumbnail-spec-0.8.0.html).

Useful for creating thumbnails in advance for big folders if you use a file manager complaint with the xdg standard (for example Gnome Nautilus aka "Files" or KDE Dolphin). 

## Supported features
- Creation of normal thumbs
- Creation of large thumbs
- Saving in XDG directory
- Saving original image last modification image in thumb PNG metadata
- Threaded image creation
- Shared repositories

## Usage
```shell script
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
  -r --recursive      Recursive scan.
  -n --normal         Generate normal thumbs.
  -l --large          Generate large thumbs.
  -j --jobs=<num>     Number of parallel jobs [default: 1]
  -o --output=<dir>   Output to custom directory
  -x --xdg            Output to XDG directory
  -s --shared         Output to shared repository directory

```

## Building
```shell script
cargo build --release
```

## See also
* [genthumbs](https://github.com/jesjimher/genthumbs) a shell script to generate the .sh_thumbnails with ImageMagic
