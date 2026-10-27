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
        if block.is_empty() || piece_length == 0 {
            return None;
        }

        // Validate block bounds within piece_length to prevent out-of-bounds or malicious memory growth
        let block_end = (begin as usize).checked_add(block.len())?;
        if block_end > piece_length as usize {
            return None;
        }

        // Guard against unbounded HashMap growth if stale piece indices were left behind
        if !self.blocks.contains_key(&index) && self.blocks.len() >= 2 {
            self.blocks.clear();
            self.blocks.shrink_to_fit();
        }

        let entry = self.blocks.entry(index).or_default();
        // Avoid duplicate or overlapping blocks for the same offset
        if !entry.iter().any(|(b, _)| *b == begin) {
            entry.push((begin, block));
        }

        // checking if, received bytes == expected piece length
        let received: usize = entry.iter().map(|(_, b)| b.len()).sum();
        if received >= piece_length as usize {
            // Sort blocks by offset and flatten into a single pre-allocated buffer
            let mut sorted = self.blocks.remove(&index).unwrap();
            if self.blocks.is_empty() && self.blocks.capacity() > 4 {
                self.blocks.shrink_to_fit();
            }
            sorted.sort_by_key(|(begin, _)| *begin);
            let mut assembled = Vec::with_capacity(piece_length as usize);
            for (_, b) in sorted {
                assembled.extend_from_slice(&b);
            }
            if assembled.len() != piece_length as usize {
                return None;
            }
            Some(assembled)
        } else {
            None
        }
    }

    /// Discard all buffered blocks for a given piece index (e.g. when a piece request stalls or chokes)
    pub fn clear_piece(&mut self, index: u32) {
        self.blocks.remove(&index);
        if self.blocks.is_empty() && self.blocks.capacity() > 4 {
            self.blocks.shrink_to_fit();
        }
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
        if total_pieces == 0 {
            return 0;
        }
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

    #[allow(dead_code)]
    pub fn received_blocks(&self, piece_index: u32) -> u32 {
        self.blocks
            .get(&piece_index)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_assembler_bounds_and_assembly() {
        let mut assembler = PieceAssembler::new();

        // Out-of-bounds block should be rejected
        assert!(assembler.add_block(0, 6, vec![1, 2, 3, 4], 8).is_none());
        assert!(assembler.blocks.is_empty());

        // Valid out-of-order blocks should assemble cleanly
        assert!(assembler.add_block(0, 4, vec![5, 6, 7, 8], 8).is_none());
        // Duplicate block at same offset should be ignored
        assert!(assembler.add_block(0, 4, vec![9, 9, 9, 9], 8).is_none());
        let full = assembler.add_block(0, 0, vec![1, 2, 3, 4], 8).unwrap();
        assert_eq!(full, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(assembler.blocks.is_empty());
    }
}
