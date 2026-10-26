use std::collections::HashMap;

use sha1::{Digest, Sha1};

pub struct PieceAssembler {
    pub blocks: HashMap<u32, Vec<(u32, Vec<u8>)>>,
}

impl PieceAssembler {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    // returns a piece when all blocks arrives.
    pub fn add_block(
        &mut self,
        index: u32,
        begin: u32,
        block: Vec<u8>,
        piece_length: u32,
    ) -> Option<Vec<u8>> {
        let entry = self.blocks.entry(index).or_default();
        // Avoid duplicate blocks for the same offset
        if !entry.iter().any(|(b, _)| *b == begin) {
            entry.push((begin, block));
        }

        // checking if, received bytes == expected piece length
        let received: usize = entry.iter().map(|(_, b)| b.len()).sum();
        if received >= piece_length as usize {
            // Sort blocks by offset and flatten
            let mut sorted = self.blocks.remove(&index).unwrap();
            sorted.sort_by_key(|(begin, _)| *begin);
            let assembled: Vec<u8> = sorted.into_iter().flat_map(|(_, b)| b).collect();
            Some(assembled)
        } else {
            None
        }
    }

    /// Discard all buffered blocks for a given piece index (e.g. when a piece request stalls or chokes)
    pub fn clear_piece(&mut self, index: u32) {
        self.blocks.remove(&index);
    }

    /// Discard all buffered blocks across all pieces and shrink capacity to free memory
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.blocks.shrink_to_fit();
    }

    pub fn actual_piece_length(
        piece_index: u32,
        total_pieces: u32,
        piece_length: u32,
        total_torrent_size: u64,
    ) -> u32 {
        let is_last = piece_index == total_pieces - 1;
        if is_last {
            let remainder = total_torrent_size % piece_length as u64;
            if remainder == 0 {
                piece_length
            } else {
                remainder as u32
            }
        } else {
            piece_length
        }
    }

    pub fn verify_piece(data: &[u8], expected_hash: &[u8]) -> bool {
        let mut hasher = Sha1::new();
        hasher.update(data);
        hasher.finalize().as_slice() == expected_hash
    }

    pub fn received_blocks(&self, piece_index: u32) -> u32 {
        self.blocks
            .get(&piece_index)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }
}
