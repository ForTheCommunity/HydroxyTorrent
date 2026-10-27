use std::path::Path;
use hydroxy_storage_io::{file_region::FileRegion, piece_io::PieceIO};
use sha1::{Digest, Sha1};
use tracing::info;

pub struct RecheckResult {
    pub bitfield: Vec<u8>,
    pub completed_pieces: usize,
    pub completed_bytes: u64,
}

pub async fn verify_torrent_data_on_disk(
    file_map: &[FileRegion],
    piece_hashes: &[u8],
    piece_length: u32,
    total_torrent_size: u64,
    total_pieces: usize,
) -> RecheckResult {
    let num_bytes = (total_pieces + 7) / 8;
    let mut bitfield = vec![0u8; num_bytes];
    let mut completed_pieces = 0;

    let any_file_exists = file_map.iter().any(|r| Path::new(&r.path).exists());
    if !any_file_exists {
        info!("No existing files found on disk for torrent. Recheck skipped (0% complete).");
        return RecheckResult {
            bitfield,
            completed_pieces: 0,
            completed_bytes: 0,
        };
    }

    info!(
        "Existing files found on disk. Rechecking {} pieces against SHA-1...",
        total_pieces
    );

    for piece_idx in 0..total_pieces {
        let piece_data_opt = match PieceIO::read_piece_from_disk(
            file_map,
            piece_idx as u32,
            piece_length,
            total_torrent_size,
        )
        .await
        {
            Ok(opt) => opt,
            Err(_) => None,
        };

        if let Some(piece_data) = piece_data_opt {
            let start = piece_idx * 20;
            let end = start + 20;
            if end <= piece_hashes.len() {
                let expected_hash = &piece_hashes[start..end];
                let mut hasher = Sha1::new();
                hasher.update(&piece_data);
                let actual_hash = hasher.finalize();

                if actual_hash.as_slice() == expected_hash {
                    let byte_idx = piece_idx / 8;
                    let bit_idx = 7 - (piece_idx % 8);
                    bitfield[byte_idx] |= 1 << bit_idx;
                    completed_pieces += 1;
                }
            }
        }
        if piece_idx % 8 == 7 {
            tokio::task::yield_now().await;
        }
    }

    let mut completed_bytes = 0u64;
    for piece_idx in 0..total_pieces {
        let byte_idx = piece_idx / 8;
        let bit_idx = 7 - (piece_idx % 8);
        if byte_idx < bitfield.len() && (bitfield[byte_idx] & (1 << bit_idx)) != 0 {
            completed_bytes += crate::piece_assembler::PieceAssembler::actual_piece_length(
                piece_idx as u32,
                total_pieces as u32,
                piece_length,
                total_torrent_size,
            ) as u64;
        }
    }
    let progress = if total_pieces > 0 {
        (completed_pieces as f64 / total_pieces as f64) * 100.0
    } else {
        0.0
    };

    info!(
        "Recheck complete: {}/{} pieces valid on disk ({:.1}%, {} bytes)",
        completed_pieces, total_pieces, progress, completed_bytes
    );

    RecheckResult {
        bitfield,
        completed_pieces,
        completed_bytes,
    }
}
