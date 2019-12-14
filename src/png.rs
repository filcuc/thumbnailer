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

use std::io::Read;

const PNG_SIGNATURE: [u8; 8] =  [137, 80, 78, 71, 13, 10, 26, 10];

struct Chunk {
    kind: String,
    length: u32,
    data: Vec<u8>,
    crc: u32
}

fn decode_chunk(file: &mut std::fs::File) -> std::result::Result<Chunk, ()> {
    let mut temp = vec![];
    let mut chunk_length: [u8; 4] = Default::default();
    let mut chunk_kind: [u8; 4] = Default::default();
    let mut chunk_crc: [u8; 4] = Default::default();
    let mut chunk_data: Vec<u8> = vec![];

    file.read_exact(&mut chunk_length);
    let chunk_length = unsafe { std::mem::transmute::<[u8; 4], u32>(chunk_length).to_be()};

    file.read_exact(&mut chunk_kind).map_err(|_| ())?;
    temp = chunk_kind.to_vec();
    let chunk_type = String::from_utf8(chunk_kind.to_vec()).map_err(|_|())?;

    if chunk_length > 0 {
        chunk_data.resize(chunk_length as usize, 0);
        file.read_exact(&mut chunk_data);
        temp.append(&mut chunk_data.clone());
    }

    file.read_exact(&mut chunk_crc).map_err(|_|())?;
    let chunk_crc = unsafe { std::mem::transmute::<[u8; 4], u32>(chunk_crc).to_be()};


    let crc = CRC::new();

    Ok(Chunk {
        length: chunk_length,
        data: chunk_data,
        kind: chunk_type,
        crc: chunk_crc
    })
}

struct CRC {
    crc_table: [u32;256]
}

impl CRC {
    fn make_crc_table() -> [u32; 256] {
        let mut result: [u32; 256] = [0; 256];

        for n in 0..256 {
            let mut c = n as u32;
            for k in 0..8 {
                if c & 1 != 0 {
                    c = 0xedb88320u32 ^ (c >> 1);
                } else {
                    c = c >> 1;
                }
            }
            result[n] = c;
        }

        result
    }

    fn new() -> CRC {
        CRC { crc_table: CRC::make_crc_table()}
    }

    fn update_crc(&self, crc: u32, buf: &Vec<u8>) -> u32 {
        let mut c = crc;

        for n in 0..buf.len() {
            let k = (c ^ (buf[n] as u32)) & 0xff;
            c = self.crc_table[k as usize] ^ (c >> 8);
        }

        c
    }

    fn crc(&self, data: &Vec<u8>) -> u32 {
        self.update_crc(0xffffffffu32, &data) ^ 0xffffffffu32
    }
}

#[cfg(test)]
mod tests {
    use crate::png::{PNG_SIGNATURE, decode_chunk, CRC};
    use std::io::{Read, Seek};
    use std::path::PathBuf;

    #[test]
    fn test_png() {
        let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("test_resources")
            .join("image.png");
        assert!(path.exists());
        assert!(path.is_file());
        let mut file = std::fs::File::open(path).unwrap();
        let mut buf: [u8; 8] = Default::default();
        assert!(file.read_exact(&mut buf).is_ok());
        assert_eq!(buf, PNG_SIGNATURE);

        loop {
            let chunk = match decode_chunk(&mut file) {
                Ok(c) => c,
                _ => break
            };
            if chunk.kind == "IEND" {
                break;
            }
        }
    }
}
