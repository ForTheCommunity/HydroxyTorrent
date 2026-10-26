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
        let piece_start = self.piece_index as u64 * self.piece_length as u64;
        let piece_end = piece_start + self.data.len() as u64;

        for region in self.file_map {
            let region_end = region.offset_in_torrent + region.length;

            // skipping files that doesn't overlap with this piece
            if piece_start >= region_end || piece_end <= region.offset_in_torrent {
                continue;
            }

            let overlap_start = piece_start.max(region.offset_in_torrent);
            let overlap_end = piece_end.min(region_end);

            let data_offset = (overlap_start - piece_start) as usize;
            let data_slice =
                &self.data[data_offset..data_offset + (overlap_end - overlap_start) as usize];
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
