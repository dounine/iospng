use crate::chunk::{Chunk, ChunkType};
use crate::error::Error;
use fast_stream::bytes::{Bytes, ValueWrite};
use fast_stream::deflate::Deflate;
use fast_stream::endian::Endian;
use fast_stream::pin::Pin;
use fast_stream::stream::Stream;
use std::io::Read;

mod chunk;
pub mod error;

pub const PNG_MAGIC_BYTES: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
pub struct U88([u8; 8]);
impl ValueWrite for U88 {
    fn write(self, _endian: &Endian) -> std::io::Result<Stream> {
        Ok(self.0.to_vec().into())
    }
}
#[derive(Debug)]
pub struct Png;
impl Png {
    pub fn is_png(magic_data: &[u8], file_length: usize) -> Result<bool, Error> {
        if magic_data != PNG_MAGIC_BYTES {
            return Ok(false);
        }
        if file_length < 8 {
            return Ok(false);
        }
        Ok(true)
    }
    pub fn restore(input: Stream, output: &mut Stream) -> Result<(), Error> {
        let mut input = input;
        let file_length = input.length() as usize;
        input.with_endian(Endian::Big);
        let mut magic = [0_u8; 8];
        input.read_exact(&mut magic)?;
        if !Self::is_png(&magic, file_length)? {
            return Error::NotIosPng.into();
        }
        let mut chunks = Chunk::parse(&mut input)?;
        if chunks.is_empty() {
            input.seek_start()?;
            output.append(&mut input)?;
            return Ok(());
        }
        if let Some(chunk) = chunks.last()
            && chunk.id != ChunkType::IEND
        {
            //IEND
            return Error::Error("missing IEND chunk".to_string()).into();
        }
        if chunks.len() == 0 {
            return Error::Error("png chunks is empty".to_string()).into();
        }
        if let Some(chunk) = chunks.first()
            && chunk.id != ChunkType::CgBI
        {
            /* "CgBI" */
            // return Error::NotIosPng.into();
            input.seek_start()?;
            output.append(&mut input)?;
            return Ok(());
        }
        let ihdr_chunk: &mut Chunk;
        if let Some(chunk) = chunks.get_mut(1) {
            if chunk.id == ChunkType::IhDr {
                ihdr_chunk = chunk;
            } else {
                return Error::Error("no IHDR chunk found".to_string()).into();
            }
        } else {
            return Error::Error("no IHDR chunk found".to_string()).into();
        }
        if ihdr_chunk.length != 13 {
            return Error::Error("IHDR chunk length incorrect".to_string()).into();
        }
        ihdr_chunk.data.set_position(4)?;
        let img_width: u32 = ihdr_chunk.data.read_value()?;
        let img_height: u32 = ihdr_chunk.data.read_value()?;
        let bit_depth: u8 = ihdr_chunk.data.read_value()?;
        let color_type: u8 = ihdr_chunk.data.read_value()?;
        let compression: u8 = ihdr_chunk.data.read_value()?;
        let filter: u8 = ihdr_chunk.data.read_value()?;
        let interlace: u8 = ihdr_chunk.data.read_value()?;
        if img_width == 0 || img_height == 0 || img_width > 2147483647 || img_height > 2147483647 {
            return Error::Error("invalid image size".to_string()).into();
        }
        if compression != 0 {
            return Error::Error(format!("unknown compression type {}", compression)).into();
        }
        if filter != 0 {
            return Error::Error(format!("unknown filter type {}", filter)).into();
        }
        if interlace != 0 && interlace != 1 {
            return Error::Error(format!("unknown interlace type {}", interlace)).into();
        }
        let bit_spp: u8;
        match color_type {
            0 if bit_depth == 1
                || bit_depth == 2
                || bit_depth == 4
                || bit_depth == 8
                || bit_depth == 16 =>
            {
                bit_spp = bit_depth;
            }
            2 if bit_depth == 8 || bit_depth == 16 => {
                bit_spp = 3 * bit_depth;
            }
            3 if bit_depth == 1 || bit_depth == 2 || bit_depth == 4 || bit_depth == 8 => {
                bit_spp = bit_depth
            }
            4 if bit_depth == 8 || bit_depth == 16 => bit_spp = 2 * bit_depth,
            6 if bit_depth == 8 || bit_depth == 16 => bit_spp = 4 * bit_depth,
            _ => {
                return Error::Error(format!(
                    "unknown color type {} with bit depth {}",
                    color_type, bit_depth
                ))
                .into();
            }
        }
        let byte_sp_line = (img_width * bit_spp as u32 + 7) / 8;
        let byte_spp = (bit_spp + 7) / 8;
        let mut row_filter_bytes = img_height;

        let starting_row = vec![0, 0, 4, 0, 2, 0, 1];
        let starting_col = vec![0, 4, 0, 2, 0, 1, 0];
        let row_increment = vec![8, 8, 8, 4, 4, 2, 2];
        let col_increment = vec![8, 8, 4, 4, 2, 2, 1];

        if interlace == 1 {
            row_filter_bytes = 0;
            for pass in 0..7 {
                // let w = (img_width - starting_col[pass] + col_increment[pass] - 1)
                //     / col_increment[pass];
                let h = (img_width - starting_row[pass] + row_increment[pass] - 1)
                    / row_increment[pass];
                row_filter_bytes += h;
            }
        }
        let idat_chunk = chunks
            .iter_mut()
            .find(|c| c.id == ChunkType::IdAt)
            .ok_or(Error::Error("no IDAT chunks found".to_string()))?;
        if idat_chunk.length == 0 {
            return Error::Error("all IDAT chunks are empty".to_string()).into();
        }
        let mut data_repack = vec![];
        /*	Swap BGR to RGB, BGRA to RGBA */
        if bit_depth == 8 && (color_type == 2 || color_type == 6) {
            // if idat_chunk
            let mut all_idat = Stream::empty(); // vec![];
            for idat_chunk in &mut chunks {
                if idat_chunk.id == ChunkType::IdAt {
                    idat_chunk.data.set_position(4)?;
                    all_idat.append(&mut idat_chunk.data)?;
                }
            }
            let out_lenth = all_idat.length();
            all_idat.seek_start()?;
            let mut data_out = all_idat;
            data_out.decompress()?;
            let mut data_out = data_out.take_data()?;
            if out_lenth <= 0 {
                return Error::Error("unspecified decompression error".to_string()).into();
            }
            if interlace == 1 {
                let mut y = 0;
                for pass in 0..7 {
                    let w = (img_width - starting_col[pass] + col_increment[pass] - 1)
                        / col_increment[pass];
                    let h = (img_height - starting_row[pass] + row_increment[pass] - 1)
                        / row_increment[pass];
                    let mut row = 0;
                    while row < h {
                        if data_out[y] > 4 {
                            return Error::Error(format!(
                                "unknown row filter type {}",
                                data_out[y]
                            ))
                            .into();
                        }
                        y += 1;
                        y += (w * byte_spp as u32) as usize;
                        row += 1;
                    }
                    let mut y = 0;
                    for pass in 0..7 {
                        let w = (img_width - starting_col[pass] + col_increment[pass] - 1)
                            / col_increment[pass];
                        let h = (img_height - starting_row[pass] + row_increment[pass] - 1)
                            / row_increment[pass];
                        let start = y;
                        row = 0;
                        while row < h {
                            y += 1;
                            let mut x = 0;
                            while x < w {
                                data_out.swap(y + 2, y);
                                y += byte_spp as usize;
                                x += 1;
                            }
                            row += 1;
                        }
                        if color_type == 6 {
                            Self::remove_row_filters(w, h, &mut data_out[start..]);
                            Self::demultiply_alpha(w, h, &mut data_out[start..]);
                            Self::apply_row_filters(w, h, &mut data_out[start..]);
                        }
                    }
                }
            } else {
                let mut y: u32 = 0;
                let end = byte_sp_line * img_height + row_filter_bytes;
                while y < end {
                    if data_out[y as usize] > 4 {
                        return Error::Error(format!(
                            "unknown row filter type {}",
                            data_out[y as usize]
                        ))
                        .into();
                    }
                    y += 1;
                    y += byte_sp_line;
                }
                let mut x: u32;
                y = 0;
                while y < end {
                    y += 1;
                    x = 0;
                    let mut b: u8;
                    while x < img_width {
                        b = data_out[y as usize + 2];
                        data_out[y as usize + 2] = data_out[y as usize];
                        data_out[y as usize] = b;
                        y += byte_spp as u32;
                        x += 1;
                    }
                }
                if color_type == 6 {
                    Self::remove_row_filters(img_width, img_height, &mut data_out);
                    Self::demultiply_alpha(img_width, img_height, &mut data_out);
                    Self::apply_row_filters(img_width, img_height, &mut data_out);
                }
            }
            let mut data = Stream::new(data_out.clone().into());
            data.compress_zlib(&fast_stream::deflate::CompressionLevel::DefaultLevel)?;
            data_repack = data.take_data()?;
            if data_repack.len() == 0 {
                return Error::Error("unspecified compression error".to_string()).into();
            }
        }

        output.with_endian(Endian::Big);
        output.write_value(U88(PNG_MAGIC_BYTES))?;

        for chunk in &mut chunks {
            chunk.data.init_crc32();
            chunk.data.hash_computer()?;
            chunk.crc32 = chunk.data.crc32_value();
        }

        let mut chunks_iter = chunks.into_iter().peekable();

        if let Some(chunk) = chunks_iter.peek()
            && chunk.id == ChunkType::CgBI
        {
            chunks_iter.next();
        }

        while let Some(chunk) = chunks_iter.peek_mut()
            && chunk.id != ChunkType::IdAt
        {
            // if chunk.id == ChunkType::IdAt {
            //     break;
            // }
            output.write_value(chunk.length)?;
            chunk.data.seek_start()?;
            output.append(&mut chunk.data)?;
            output.write_value(chunk.crc32)?;
            chunks_iter.next();
        }

        let repack_idat_size: usize = 524288; //512kb
        let repack_length = data_repack.len();
        if repack_length > 0 {
            let mut write_block_size = 0;
            let idat_bytes = vec![b'I', b'D', b'A', b'T'];
            data_repack.splice(0..0, idat_bytes);
            while write_block_size < repack_length {
                let mut crc32 = Stream::new(data_repack.clone().into());
                crc32.init_crc32();
                crc32.hash_computer()?;
                let crc32 = crc32.crc32_value();
                if repack_length - write_block_size > repack_idat_size {
                    output.write_value(repack_idat_size as u32)?;
                    output.extend_from_slice(&data_repack)?;
                    output.write_value(crc32)?;
                    write_block_size += repack_idat_size;
                } else {
                    let value = (repack_length - write_block_size) as u32;
                    output.write_value(value)?;
                    output.extend_from_slice(&data_repack)?;
                    output.write_value(crc32)?;
                    write_block_size = repack_length;
                }
            }
            while let Some(chunk) = chunks_iter.peek() {
                if chunk.id == ChunkType::IdAt {
                    chunks_iter.next();
                    break;
                }
            }
        } else {
            while let Some(chunk) = chunks_iter.peek_mut() {
                if chunk.id == ChunkType::IdAt {
                    output.write_value(chunk.length)?;
                    output.append(&mut chunk.data)?;
                    output.write_value(chunk.crc32)?;
                    chunks_iter.next();
                }
            }
        }
        /* output remaining chunks */
        for mut chunk in chunks_iter {
            output.write_value(chunk.length)?;
            output.append(&mut chunk.data)?;
            output.write_value(chunk.crc32)?;
        }
        Ok(())
    }
    fn remove_row_filters(width: u32, height: u32, data: &mut [u8]) {
        let mut src_index = 0;
        for y in 0..height {
            let row_filter: u8 = data[src_index];
            src_index += 1;
            match row_filter {
                0 => {
                    //None
                    break;
                }
                1 => {
                    //Sub
                    for x in 4..(4 * width as usize) {
                        data[src_index + x] += data[src_index + x - 4];
                    }
                    break;
                }
                2 => {
                    //Up
                    let up_ptr_index = src_index - 4 * width as usize - 1;
                    if y > 0 {
                        for x in 0..4 * width as usize {
                            data[src_index + x] += data[up_ptr_index + x];
                        }
                    }
                    break;
                }
                3 => {
                    //Average
                    let up_ptr_index = src_index - 4 * width as usize - 1;
                    if y > 0 {
                        for x in 4..4 * width as usize {
                            data[src_index + x] += data[src_index + x - 4] >> 1
                        }
                    } else {
                        data[src_index] += data[up_ptr_index] >> 1;
                        for x in 4..4 * width as usize {
                            data[src_index + x] +=
                                (data[up_ptr_index + x] + data[up_ptr_index + x - 4]) >> 1;
                        }
                    }
                    break;
                }
                4 => {
                    let up_ptr_index = src_index - 4 * width as usize - 1;
                    for x in 0..4 * width as usize {
                        let mut left_pix = 0;
                        let mut top_pix = 0;
                        let mut top_left_pix = 0;
                        if x > 0 {
                            left_pix = data[src_index + x - 4];
                        }
                        if y > 0 {
                            top_pix = data[up_ptr_index + x];
                            if x >= 4 {
                                top_left_pix = data[up_ptr_index + x - 4];
                            }
                        }
                        let p = left_pix + top_pix - top_left_pix;
                        let pa = (p as i8 - left_pix as i8).abs();
                        let pb = (p as i8 - top_pix as i8).abs();
                        let pc = (p as i8 - top_left_pix as i8).abs();
                        let mut value = top_left_pix;
                        if pa <= pb && pa <= pc {
                            value = left_pix;
                        } else if pb <= pc {
                            value = top_pix;
                        }
                        data[src_index + x] += value;
                    }
                }
                _ => {}
            }
            src_index += 4 * width as usize;
        }
    }
    fn demultiply_alpha(with: u32, height: u32, data: &mut [u8]) {
        let mut src_index = 0;
        for _ in 0..height as usize {
            src_index += 1;
            for x in (0..with as usize).step_by(4) {
                let value = data[src_index + x + 3];
                if value > 0 {
                    data[src_index + x] =
                        ((data[src_index + x] as u32 * 255 + (data[src_index + x + 3] as u32 >> 1))
                            / data[src_index + x + 3] as u32) as u8;
                    data[src_index + x + 1] = ((data[src_index + x + 1] as u32 * 255
                        + (data[src_index + x + 3] as u32 >> 1))
                        / data[src_index + x + 3] as u32)
                        as u8;
                    data[src_index + x + 2] = ((data[src_index + x + 2] as u32 * 255
                        + (data[src_index + x + 3] as u32 >> 1))
                        / data[src_index + x + 3] as u32)
                        as u8;
                }
            }
            src_index += 4 * with as usize;
        }
    }
    fn apply_row_filters(width: u32, height: u32, data: &mut [u8]) {
        let mut src_index = 0;
        for y in 0..height as usize {
            let row_filter: u8 = data[src_index];
            src_index += 1;
            match row_filter {
                0 => {
                    break;
                }
                1 => {
                    for x in (4..4 * width as usize - 1).rev() {
                        data[src_index + x] -= data[src_index + x - 4];
                    }
                }
                2 => {
                    if y > 0 {
                        let up_ptr_index = src_index - 1;
                        for x in (0..4 * width as usize - 1).rev() {
                            data[src_index + x] -= data[up_ptr_index + x];
                        }
                    }
                    break;
                }
                3 => {
                    let up_ptr_index = src_index - 4 * width as usize - 1;
                    if y == 0 {
                        for x in (4..4 * width as usize - 1).rev() {
                            data[src_index + x] -= data[src_index + x - 4] >> 1;
                        }
                    } else {
                        data[src_index] -= data[up_ptr_index] >> 1;
                        for x in (4..4 * width as usize - 1).rev() {
                            data[src_index + x] -=
                                (data[up_ptr_index + x] + data[src_index + x - 4]) >> 1;
                        }
                    }
                    break;
                }
                4 => {
                    let up_ptr_index = src_index - 1;
                    for x in (0..4 * width as usize - 1).rev() {
                        let mut left_pix = 0;
                        let mut top_pix = 0;
                        let mut top_left_pix = 0;
                        if x > 0 {
                            left_pix = data[src_index + x - 4];
                        }
                        if y > 0 {
                            top_pix = data[up_ptr_index];
                            if x >= 4 {
                                top_left_pix = data[up_ptr_index + x - 4];
                            }
                        }
                        let p = left_pix + top_pix + top_left_pix;
                        let pa = (p as i8 - left_pix as i8).abs();
                        let pb = (p as i8 - top_pix as i8).abs();
                        let pc = (p as i8 - top_left_pix as i8).abs();
                        let mut value = top_left_pix;
                        if pa <= pb && pa <= pc {
                            value = left_pix;
                        } else if pb <= pc {
                            value = top_pix;
                        }
                        data[src_index + x] -= value;
                    }
                }
                _ => {}
            }
            src_index += 4 * width as usize;
        }
    }
}
#[cfg(test)]
mod test {
    #[test]
    fn test_vec() {
        let a: i8 = 8;
        let b: i8 = 20;
        let c = (a - b).abs();
        dbg!(c);
        // let mut data = vec![1, 2, 3];
        // data[1] += 8;
        // let mut next_data = &mut data[1..];
        // dbg!(next_data);
    }
}
