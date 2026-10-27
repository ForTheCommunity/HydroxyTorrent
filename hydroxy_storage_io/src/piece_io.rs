use std::io::{Read, Seek, SeekFrom, Write};

use crate::{errors::StorageIOError, file_region::FileRegion};

fn run_disk_io<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

pub struct PieceIO {
    pub file_map: Vec<FileRegion>,
    pub piece_index: u32,
    pub piece_length: u32,
    pub data: Vec<u8>,
}

impl PieceIO {
    pub async fn write_to_disk(self) -> Result<(), StorageIOError> {
        Self::write_piece_to_disk(
            &self.file_map,
            self.piece_index,
            self.piece_length,
            &self.data,
        )
        .await
    }

    pub async fn write_piece_to_disk(
        file_map: &[FileRegion],
        piece_index: u32,
        piece_length: u32,
        data: &[u8],
    ) -> Result<(), StorageIOError> {
        run_disk_io(|| {
            let piece_start = piece_index as u64 * piece_length as u64;
            let piece_end = piece_start + data.len() as u64;

            for region in file_map {
                let region_end = region.offset_in_torrent + region.length;

                // skipping files that doesn't overlap with this piece
                if piece_start >= region_end || piece_end <= region.offset_in_torrent {
                    continue;
                }

                let overlap_start = piece_start.max(region.offset_in_torrent);
                let overlap_end = piece_end.min(region_end);

                let data_offset = (overlap_start - piece_start) as usize;
                let data_slice =
                    &data[data_offset..data_offset + (overlap_end - overlap_start) as usize];
                let file_offset = overlap_start - region.offset_in_torrent;

                // creating parent dirs if not exists.
                let path = std::path::Path::new(&region.path);
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)?;
                    }
                }

                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(path)?;

                file.seek(SeekFrom::Start(file_offset))?;
                file.write_all(data_slice)?;
            }

            Ok(())
        })
    }

    /// Reads only a specific `[begin .. begin + length]` sub-block of a piece from disk,
    /// avoiding allocating and reading the entire multi-MB piece for each 16KB peer upload request.
    pub async fn read_block_from_disk(
        file_map: &[FileRegion],
        piece_index: u32,
        begin: u32,
        length: u32,
        piece_length: u32,
        total_torrent_size: u64,
    ) -> Result<Option<Vec<u8>>, StorageIOError> {
        if length == 0 {
            return Ok(None);
        }

        let piece_start = piece_index as u64 * piece_length as u64;
        if piece_start >= total_torrent_size {
            return Ok(None);
        }

        let piece_size = (piece_length as u64).min(total_torrent_size - piece_start);
        let block_end_in_piece = (begin as u64).saturating_add(length as u64);
        if block_end_in_piece > piece_size {
            return Ok(None);
        }

        run_disk_io(|| {
            let block_start = piece_start + begin as u64;
            let block_end = block_start + length as u64;
            let mut block_buffer = vec![0u8; length as usize];

            for region in file_map {
                let region_end = region.offset_in_torrent + region.length;
                if block_start >= region_end || block_end <= region.offset_in_torrent {
                    continue;
                }

                let path = std::path::Path::new(&region.path);
                let overlap_start = block_start.max(region.offset_in_torrent);
                let overlap_end = block_end.min(region_end);
                let overlap_len = (overlap_end - overlap_start) as usize;
                let data_offset = (overlap_start - block_start) as usize;
                let file_offset = overlap_start - region.offset_in_torrent;

                let mut file = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(_) => return Ok(None),
                };

                if file.seek(SeekFrom::Start(file_offset)).is_err() {
                    return Ok(None);
                }

                if file
                    .read_exact(&mut block_buffer[data_offset..data_offset + overlap_len])
                    .is_err()
                {
                    return Ok(None);
                }
            }

            Ok(Some(block_buffer))
        })
    }

    pub async fn read_piece_from_disk(
        file_map: &[FileRegion],
        piece_index: u32,
        piece_length: u32,
        total_torrent_size: u64,
    ) -> Result<Option<Vec<u8>>, StorageIOError> {
        let piece_start = piece_index as u64 * piece_length as u64;
        if piece_start >= total_torrent_size {
            return Ok(None);
        }

        let piece_size = (piece_length as u64).min(total_torrent_size - piece_start) as usize;
        Self::read_block_from_disk(
            file_map,
            piece_index,
            0,
            piece_size as u32,
            piece_length,
            total_torrent_size,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_piece_and_read_block_across_files() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hydroxy_piece_io_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let f1 = temp_dir.join("file1.bin");
        let f2 = temp_dir.join("file2.bin");

        let file_map = vec![
            FileRegion {
                path: f1.to_string_lossy().to_string(),
                offset_in_torrent: 0,
                length: 10,
            },
            FileRegion {
                path: f2.to_string_lossy().to_string(),
                offset_in_torrent: 10,
                length: 10,
            },
        ];

        let piece_data: Vec<u8> = (0..16u8).collect();
        PieceIO::write_piece_to_disk(&file_map, 0, 16, &piece_data)
            .await
            .unwrap();

        // Read a 6-byte block spanning the boundary between file1 and file2 (offset 8..14)
        let block = PieceIO::read_block_from_disk(&file_map, 0, 8, 6, 16, 20)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(block, vec![8, 9, 10, 11, 12, 13]);

        // Out-of-bounds block request should return None safely without allocating
        let oob = PieceIO::read_block_from_disk(&file_map, 0, 12, 8, 16, 20)
            .await
            .unwrap();
        assert!(oob.is_none());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
