use std::io::SeekFrom;

use tokio::{
    fs::{self},
    io::{AsyncSeekExt, AsyncWriteExt},
};

use crate::{errors::StorageIOError, file_region::FileRegion};

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
            let data_slice = &data[data_offset..data_offset + (overlap_end - overlap_start) as usize];
            let file_offset = overlap_start - region.offset_in_torrent;

            // creating parent dirs if not exists.
            if let Some(parent) = std::path::Path::new(&region.path).parent() {
                fs::create_dir_all(parent).await?;
            };

            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&region.path)
                .await?;

            file.seek(SeekFrom::Start(file_offset)).await?;
            file.write_all(data_slice).await?;
            file.flush().await?;
        }

        Ok(())
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

        let block_start = piece_start + begin as u64;
        let block_end = block_start + length as u64;
        let mut block_buffer = vec![0u8; length as usize];

        for region in file_map {
            let region_end = region.offset_in_torrent + region.length;
            if block_start >= region_end || block_end <= region.offset_in_torrent {
                continue;
            }

            let path = std::path::Path::new(&region.path);
            if !path.exists() {
                return Ok(None);
            }

            let overlap_start = block_start.max(region.offset_in_torrent);
            let overlap_end = block_end.min(region_end);
            let overlap_len = (overlap_end - overlap_start) as usize;
            let data_offset = (overlap_start - block_start) as usize;
            let file_offset = overlap_start - region.offset_in_torrent;

            let mut file = match fs::File::open(path).await {
                Ok(f) => f,
                Err(_) => return Ok(None),
            };

            let metadata = match file.metadata().await {
                Ok(m) => m,
                Err(_) => return Ok(None),
            };

            if metadata.len() < file_offset + overlap_len as u64 {
                return Ok(None);
            }

            if file.seek(SeekFrom::Start(file_offset)).await.is_err() {
                return Ok(None);
            }

            use tokio::io::AsyncReadExt;
            if file
                .read_exact(&mut block_buffer[data_offset..data_offset + overlap_len])
                .await
                .is_err()
            {
                return Ok(None);
            }
        }

        Ok(Some(block_buffer))
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
        let piece_end = piece_start + piece_size as u64;
        let mut piece_buffer = vec![0u8; piece_size];

        for region in file_map {
            let region_end = region.offset_in_torrent + region.length;
            if piece_start >= region_end || piece_end <= region.offset_in_torrent {
                continue;
            }

            let path = std::path::Path::new(&region.path);
            if !path.exists() {
                return Ok(None);
            }

            let overlap_start = piece_start.max(region.offset_in_torrent);
            let overlap_end = piece_end.min(region_end);
            let overlap_len = (overlap_end - overlap_start) as usize;
            let data_offset = (overlap_start - piece_start) as usize;
            let file_offset = overlap_start - region.offset_in_torrent;

            let mut file = match fs::File::open(path).await {
                Ok(f) => f,
                Err(_) => return Ok(None),
            };

            let metadata = match file.metadata().await {
                Ok(m) => m,
                Err(_) => return Ok(None),
            };

            if metadata.len() < file_offset + overlap_len as u64 {
                return Ok(None);
            }

            if let Err(_) = file.seek(SeekFrom::Start(file_offset)).await {
                return Ok(None);
            }

            use tokio::io::AsyncReadExt;
            if let Err(_) = file.read_exact(&mut piece_buffer[data_offset..data_offset + overlap_len]).await {
                return Ok(None);
            }
        }

        Ok(Some(piece_buffer))
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
