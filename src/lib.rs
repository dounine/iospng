use crate::chunk::{Chunk, ChunkType};
use crate::error::Error;
use binrw::io::{Read, Seek, Write};
use binrw::{BinReaderExt, BinWriterExt};
use miniz_oxide::deflate::{CompressionLevel, compress_to_vec_zlib};
use miniz_oxide::inflate::stream::{decompress_stream};

mod chunk;
pub mod error;

pub const PNG_MAGIC_BYTES: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
pub struct Png<T> {
    _marker: std::marker::PhantomData<T>,
}
impl<T> Png<T>
where
    T: Read + Write + Seek + Send + Default,
{
    pub fn is_png(magic_data: &[u8], file_length: u64) -> Result<bool, Error> {
        if magic_data != PNG_MAGIC_BYTES {
            return Ok(false);
        }
        if file_length < 8 {
            return Ok(false);
        }
        Ok(true)
    }
    pub fn restore<R, W>(
        mut reader: R,
        writer: &mut W,
    ) -> impl Future<Output = Result<(), Error>> + Send
    where
        R: Read + Seek + Send,
        W: Write + Seek + Send,
    {
        async move {
            let pos = reader.position().await?;
            let file_length = reader.seek_end().await?;
            reader.set_position(pos).await?;

            let magic: [u8; 8] = reader.read_be().await?;

            if !Self::is_png(&magic, file_length)? {
                return Error::NotIosPng.into();
            }
            let mut chunks = Chunk::parse(&mut reader, file_length).await?;
            if chunks.is_empty() {
                reader.seek_start().await?;
                binrw::io::copy(&mut reader, writer).await?;
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
                reader.seek_start().await?;
                binrw::io::copy(&mut reader, writer).await?;
                return Ok(());
            }
            let ihdr_chunk: &mut Chunk<T>;
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
            ihdr_chunk.data.set_position(4).await?;
            let img_width: u64 = ihdr_chunk.data.read_be::<u32>().await? as u64;
            let img_height: u64 = ihdr_chunk.data.read_be::<u32>().await? as u64;
            let bit_depth: u8 = ihdr_chunk.data.read_be().await?;
            let color_type: u8 = ihdr_chunk.data.read_be().await?;
            let compression: u8 = ihdr_chunk.data.read_be().await?;
            let filter: u8 = ihdr_chunk.data.read_be().await?;
            let interlace: u8 = ihdr_chunk.data.read_be().await?;
            if img_width == 0
                || img_height == 0
                || img_width > 2147483647
                || img_height > 2147483647
            {
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
            let byte_sp_line = (img_width * bit_spp as u64 + 7) / 8;
            let byte_spp = (bit_spp + 7) / 8;
            let mut row_filter_bytes = img_height;

            let starting_row: Vec<u64> = vec![0, 0, 4, 0, 2, 0, 1];
            let starting_col: Vec<u64> = vec![0, 4, 0, 2, 0, 1, 0];
            let row_increment: Vec<u64> = vec![8, 8, 8, 4, 4, 2, 2];
            let col_increment: Vec<u64> = vec![8, 8, 4, 4, 2, 2, 1];

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
                let mut all_idat = T::default(); // Stream::empty(); // vec![];
                for idat_chunk in &mut chunks {
                    if idat_chunk.id == ChunkType::IdAt {
                        idat_chunk.data.set_position(4).await?;
                        binrw::io::copy(&mut idat_chunk.data, &mut all_idat).await?;
                    }
                }
                let out_lenth = all_idat.seek_end().await?;
                all_idat.seek_start().await?;
                let mut data_out = T::default();
                decompress_stream(all_idat, &mut data_out)
                    .await
                    .map_err(|_| Error::Error("failed to decompress data".to_string()))?;
                data_out.seek_start().await?;
                // un_compress_data1.read_to_end(&mut un_compress_data).await?;
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
                            data_out.set_position(y as u64).await?;
                            let value = data_out.read_byte().await?;
                            if value > 4 {
                                return Error::Error(format!("unknown row filter type {}", value))
                                    .into();
                            }
                            y += 1;
                            y += w * byte_spp as u64;
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
                                    data_out.set_position(y + 2).await?;
                                    let y_2_value = data_out.read_byte().await?;
                                    data_out.set_position(y).await?;
                                    let y_value = data_out.read_byte().await?;
                                    data_out.set_position(y).await?;
                                    data_out.write_all(&[y_2_value]).await?;
                                    data_out.set_position(y + 2).await?;
                                    data_out.write_all(&[y_value]).await?;
                                    // data_out.swap(y + 2, y);
                                    y += byte_spp as u64;
                                    x += 1;
                                }
                                row += 1;
                            }
                            if color_type == 6 {
                                Self::remove_row_filters(w, h, &mut data_out, start).await?;
                                Self::demultiply_alpha(w, h, &mut data_out, start).await?;
                                Self::apply_row_filters(w, h, &mut data_out, start).await?;
                            }
                        }
                    }
                } else {
                    let mut y: u64 = 0;
                    let end = (byte_sp_line as u64 * img_height) + row_filter_bytes;
                    while y < end {
                        data_out.set_position(y).await?;
                        let val = data_out.read_byte().await?;
                        if val > 4 {
                            return Error::Error(format!("unknown row filter type {}", val)).into();
                        }
                        y += 1;
                        y += byte_sp_line as u64;
                    }
                    let mut x: u64;
                    y = 0;
                    while y < end {
                        y += 1;
                        x = 0;
                        while x < img_width {
                            data_out.set_position(y + 2).await?;
                            let b = data_out.read_byte().await?;

                            data_out.set_position(y).await?;
                            let r = data_out.read_byte().await?;

                            data_out.set_position(y).await?;
                            data_out.write_all(&[b]).await?;

                            data_out.set_position(y + 2).await?;
                            data_out.write_all(&[r]).await?;

                            y += byte_spp as u64;
                            x += 1;
                        }
                    }
                    if color_type == 6 {
                        Self::remove_row_filters(img_width, img_height, &mut data_out, 0).await?;
                        Self::demultiply_alpha(img_width, img_height, &mut data_out, 0).await?;
                        Self::apply_row_filters(img_width, img_height, &mut data_out, 0).await?;
                    }
                }
                data_out.seek_start().await?;
                let mut buffer = vec![];
                data_out.read_to_end(&mut buffer).await?;
                (data_repack, _) = Self::compress_zlib(&buffer, &CompressionLevel::DefaultLevel)?;
                if data_repack.len() == 0 {
                    return Error::Error("unspecified compression error".to_string()).into();
                }
            }
            writer.write_be(&PNG_MAGIC_BYTES).await?;

            for chunk in &mut chunks {
                chunk.data.seek_start().await?;
                let crc32_value = Self::crc32_value(&mut chunk.data).await?;
                chunk.crc32 = crc32_value;
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
                writer.write_be(&chunk.length).await?;
                chunk.data.seek_start().await?;
                binrw::io::copy(&mut chunk.data, writer).await?;
                writer.write_be(&chunk.crc32).await?;
                chunks_iter.next();
            }

            let repack_idat_size: usize = 1024 * 512; //512kb
            let repack_length = data_repack.len();
            if repack_length > 0 {
                let mut write_block_size = 0;
                let idat_bytes = vec![b'I', b'D', b'A', b'T'];
                data_repack.splice(0..0, idat_bytes);
                while write_block_size < repack_length {
                    // let mut crc32 = T::default(); // Stream::new(data_repack.clone().into());
                    // crc32.write_all(&data_repack).await?;
                    let mut hasher = crc32fast::Hasher::new();
                    hasher.update(&data_repack);
                    let crc32 = hasher.finalize();
                    // let crc32 = Self::crc32_value(&mut crc32).await?;
                    if repack_length - write_block_size > repack_idat_size {
                        writer.write_be(&(repack_idat_size as u32)).await?;
                        writer.write_all(&data_repack).await?;
                        writer.write_be(&crc32).await?;
                        write_block_size += repack_idat_size;
                    } else {
                        let value = (repack_length - write_block_size) as u32;
                        writer.write_be(&value).await?;
                        writer.write_all(&data_repack).await?;
                        writer.write_be(&crc32).await?;
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
                        writer.write_be(&chunk.length).await?;
                        binrw::io::copy(&mut chunk.data, writer).await?;
                        writer.write_be(&chunk.crc32).await?;
                        chunks_iter.next();
                    }
                }
            }
            /* output remaining chunks */
            for mut chunk in chunks_iter {
                writer.write_be(&chunk.length).await?;
                binrw::io::copy(&mut chunk.data, writer).await?;
                writer.write_be(&chunk.crc32).await?;
            }
            Ok(())
        }
    }
    fn crc32_value<D: Read + Seek + Send>(
        data: &mut D,
    ) -> impl Future<Output = std::io::Result<u32>> + Send {
        async move {
            let mut hasher = crc32fast::Hasher::new();
            data.seek_start().await?;
            loop {
                let mut bytes = [0_u8; 1024 * 8];
                let size = data.read(&mut bytes).await?;
                if size == 0 {
                    break;
                }
                let slice = &bytes[..size];
                hasher.update(slice);
            }
            Ok(hasher.finalize())
        }
    }
    fn compress_zlib(data: &[u8], level: &CompressionLevel) -> std::io::Result<(Vec<u8>, u64)> {
        let level = match level {
            CompressionLevel::NoCompression => 0,
            CompressionLevel::BestSpeed => 1,
            CompressionLevel::BestCompression => 9,
            CompressionLevel::UberCompression => 10,
            CompressionLevel::DefaultLevel => 6,
            CompressionLevel::DefaultCompression => 0,
        };
        let compress_data = if data.len() > 0 {
            compress_to_vec_zlib(&data, level)
        } else {
            vec![]
        };
        let length = compress_data.len() as u64;
        Ok((compress_data, length))
    }

    fn remove_row_filters(
        width: u64,
        height: u64,
        data: &mut T,
        start: u64,
    ) -> impl Future<Output = std::io::Result<()>> + Send {
        async move {
            let mut src_index = 0;
            let bpp = 4;
            let row_bytes = width * 4;

            for y in 0..height {
                data.set_position(src_index + start).await?;
                let row_filter = data.read_byte().await?;
                src_index += 1;

                let current_row_start = start + src_index;
                let prev_row_start = if y > 0 {
                    current_row_start - (row_bytes + 1)
                } else {
                    0
                };

                match row_filter {
                    0 => {
                        // None
                    }
                    1 => {
                        // Sub
                        for x in bpp..row_bytes {
                            data.set_position(current_row_start + x).await?;
                            let mut val = data.read_byte().await?;

                            data.set_position(current_row_start + x - bpp).await?;
                            let prev = data.read_byte().await?;

                            val = val.wrapping_add(prev);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[val]).await?;
                        }
                    }
                    2 => {
                        // Up
                        if y > 0 {
                            for x in 0..row_bytes {
                                data.set_position(current_row_start + x).await?;
                                let mut val = data.read_byte().await?;

                                data.set_position(prev_row_start + x).await?;
                                let prior = data.read_byte().await?;

                                val = val.wrapping_add(prior);

                                data.set_position(current_row_start + x).await?;
                                data.write_all(&[val]).await?;
                            }
                        }
                    }
                    3 => {
                        // Average
                        for x in 0..row_bytes {
                            let mut left = 0;
                            if x >= bpp {
                                data.set_position(current_row_start + x - bpp)
                                    .await?;
                                left = data.read_byte().await?;
                            }

                            let mut up = 0;
                            if y > 0 {
                                data.set_position(prev_row_start + x).await?;
                                up = data.read_byte().await?;
                            }

                            let avg = ((left as u16 + up as u16) >> 1) as u8;

                            data.set_position(current_row_start + x).await?;
                            let mut val = data.read_byte().await?;

                            val = val.wrapping_add(avg);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[val]).await?;
                        }
                    }
                    4 => {
                        // Paeth
                        for x in 0..row_bytes {
                            let mut a = 0;
                            if x >= bpp {
                                data.set_position(current_row_start + x - bpp).await?;
                                a = data.read_byte().await?;
                            }

                            let mut b = 0;
                            if y > 0 {
                                data.set_position(prev_row_start + x).await?;
                                b = data.read_byte().await?;
                            }

                            let mut c = 0;
                            if y > 0 && x >= bpp {
                                data.set_position(prev_row_start + x - bpp).await?;
                                c = data.read_byte().await?;
                            }

                            let p = (a as i16) + (b as i16) - (c as i16);
                            let pa = (p - (a as i16)).abs();
                            let pb = (p - (b as i16)).abs();
                            let pc = (p - (c as i16)).abs();

                            let predictor = if pa <= pb && pa <= pc {
                                a
                            } else if pb <= pc {
                                b
                            } else {
                                c
                            };

                            data.set_position(current_row_start + x).await?;
                            let mut val = data.read_byte().await?;

                            val = val.wrapping_add(predictor);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[val]).await?;
                        }
                    }
                    _ => {}
                }
                src_index += row_bytes;
            }
            Ok(())
        }
    }
    fn demultiply_alpha(
        width: u64,
        height: u64,
        data: &mut T,
        start: u64,
    ) -> impl Future<Output = std::io::Result<()>> + Send {
        async move {
            let mut src_index = 0;
            let row_bytes = width * 4;

            for _ in 0..height {
                src_index += 1;
                let current_row_start = start + src_index;

                for x in (0..row_bytes).step_by(4) {
                    data.set_position(current_row_start + x + 3).await?;
                    let alpha = data.read_byte().await?;

                    if alpha > 0 {
                        data.set_position(current_row_start + x).await?;
                        let mut rgb = [0u8; 3];
                        data.read_exact(&mut rgb).await?;

                        let r = rgb[0];
                        let g = rgb[1];
                        let b = rgb[2];

                        let alpha_u32 = alpha as u32;
                        let half_alpha = alpha_u32 >> 1;

                        let new_r = ((r as u32 * 255 + half_alpha) / alpha_u32) as u8;
                        let new_g = ((g as u32 * 255 + half_alpha) / alpha_u32) as u8;
                        let new_b = ((b as u32 * 255 + half_alpha) / alpha_u32) as u8;

                        data.set_position(current_row_start + x).await?;
                        data.write_all(&[new_r, new_g, new_b]).await?;
                    }
                }
                src_index += row_bytes;
            }
            Ok(())
        }
    }
    fn apply_row_filters(
        width: u64,
        height: u64,
        data: &mut T,
        start: u64,
    ) -> impl Future<Output = std::io::Result<()>> + Send {
        async move {
            let mut src_index = 0;
            let bpp = 4;
            let row_bytes = width * 4;

            for y in 0..height {
                data.set_position(src_index + start).await?;
                let row_filter = data.read_byte().await?;
                src_index += 1;

                let current_row_start = start + src_index;
                let prev_row_start = if y > 0 {
                    current_row_start - (row_bytes + 1)
                } else {
                    0
                };

                match row_filter {
                    0 => {
                        // None
                    }
                    1 => {
                        // Sub
                        for x in (bpp..row_bytes).rev() {
                            data.set_position(current_row_start + x).await?;
                            let val = data.read_byte().await?;

                            data.set_position(current_row_start + x - bpp).await?;
                            let left = data.read_byte().await?;

                            let new_val = val.wrapping_sub(left);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[new_val]).await?;
                        }
                    }
                    2 => {
                        // Up
                        if y > 0 {
                            for x in 0..row_bytes {
                                data.set_position(current_row_start + x).await?;
                                let val = data.read_byte().await?;

                                data.set_position(prev_row_start + x).await?;
                                let up = data.read_byte().await?;

                                let new_val = val.wrapping_sub(up);

                                data.set_position(current_row_start + x).await?;
                                data.write_all(&[new_val]).await?;
                            }
                        }
                    }
                    3 => {
                        // Average
                        for x in (0..row_bytes).rev() {
                            data.set_position(current_row_start + x).await?;
                            let val = data.read_byte().await?;

                            let mut left = 0;
                            if x >= bpp {
                                data.set_position(current_row_start + x - bpp).await?;
                                left = data.read_byte().await?;
                            }

                            let mut up = 0;
                            if y > 0 {
                                data.set_position(prev_row_start + x).await?;
                                up = data.read_byte().await?;
                            }

                            let avg = ((left as u16 + up as u16) >> 1) as u8;
                            let new_val = val.wrapping_sub(avg);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[new_val]).await?;
                        }
                    }
                    4 => {
                        // Paeth
                        for x in (0..row_bytes).rev() {
                            data.set_position(current_row_start + x).await?;
                            let val = data.read_byte().await?;

                            let mut a = 0; // left
                            if x >= bpp {
                                data.set_position(current_row_start + x - bpp).await?;
                                a = data.read_byte().await?;
                            }

                            let mut b = 0; // up
                            if y > 0 {
                                data.set_position(prev_row_start + x).await?;
                                b = data.read_byte().await?;
                            }

                            let mut c = 0; // up-left
                            if y > 0 && x >= bpp {
                                data.set_position(prev_row_start + x - bpp).await?;
                                c = data.read_byte().await?;
                            }

                            let p = (a as i16) + (b as i16) - (c as i16);
                            let pa = (p - (a as i16)).abs();
                            let pb = (p - (b as i16)).abs();
                            let pc = (p - (c as i16)).abs();

                            let predictor = if pa <= pb && pa <= pc {
                                a
                            } else if pb <= pc {
                                b
                            } else {
                                c
                            };

                            let new_val = val.wrapping_sub(predictor);

                            data.set_position(current_row_start + x).await?;
                            data.write_all(&[new_val]).await?;
                        }
                    }
                    _ => {}
                }
                src_index += row_bytes;
            }
            Ok(())
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
