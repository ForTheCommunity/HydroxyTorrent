use std::net::IpAddr;

pub struct PieceAllocator {
    // one vec index = piece index
    pub pieces: Vec<PieceStatus>,
    // rarest first strategy for healthy swarm.
    pub piece_availability: Vec<u32>,
    pub sequential: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceStatus {
    Missing,
    Complete,
    Downloading { peer_ip: IpAddr },
}

impl PieceAllocator {
    #[allow(dead_code)]
    pub fn new(total_pieces: usize) -> Self {
        Self {
            pieces: vec![PieceStatus::Missing; total_pieces],
            piece_availability: vec![0u32; total_pieces],
            sequential: false,
        }
    }

    pub fn register_bitfield(&mut self, bitfield: &[u8], total_pieces: usize) {
        for piece_index in 0..total_pieces.min(self.piece_availability.len()) {
            if Self::check_piece(bitfield, piece_index) {
                self.piece_availability[piece_index] =
                    self.piece_availability[piece_index].saturating_add(1);
            }
        }
    }

    // picking the rarest piece that a peer has (or lowest index if sequential)
    // & marking it as downloading
    #[allow(dead_code)]
    pub fn allocate_piece(
        &mut self,
        peer_bitfield: &[u8],
        peer_ip: &IpAddr,
        total_pieces: usize,
    ) -> Option<usize> {
        self.allocate_piece_excluding(peer_bitfield, peer_ip, total_pieces, None)
    }

    // picking the rarest piece that a peer has (excluding a specific piece index if provided)
    // & marking it as downloading
    pub fn allocate_piece_excluding(
        &mut self,
        peer_bitfield: &[u8],
        peer_ip: &IpAddr,
        total_pieces: usize,
        exclude_piece: Option<usize>,
    ) -> Option<usize> {
        let limit = total_pieces.min(self.pieces.len());
        if self.sequential {
            for i in 0..limit {
                if Some(i) == exclude_piece {
                    continue;
                }
                if matches!(self.pieces[i], PieceStatus::Missing) && Self::check_piece(peer_bitfield, i)
                {
                    self.pieces[i] = PieceStatus::Downloading {
                        peer_ip: *peer_ip,
                    };
                    return Some(i);
                }
            }
            return None;
        }

        let mut best: Option<(usize, u32)> = None;

        for i in 0..limit {
            if Some(i) == exclude_piece {
                continue;
            }
            if matches!(self.pieces[i], PieceStatus::Missing) && Self::check_piece(peer_bitfield, i)
            {
                let availability = self.piece_availability[i];
                if best.is_none() || availability < best.unwrap().1 {
                    best = Some((i, availability))
                }
            }
        }

        if let Some((idx, _)) = best {
            self.pieces[idx] = PieceStatus::Downloading {
                peer_ip: *peer_ip,
            };
            Some(idx)
        } else {
            None
        }
    }

    // release piece when downloading is interrupted.
    // marking piece to Missing...
    pub fn release_piece(&mut self, piece_index: usize) {
        if piece_index < self.pieces.len()
            && matches!(self.pieces[piece_index], PieceStatus::Downloading { .. })
        {
            self.pieces[piece_index] = PieceStatus::Missing;
        }
    }

    // Reset all in-progress (Downloading) pieces back to Missing (e.g. when pausing or shutting down workers).
    pub fn reset_in_progress(&mut self) {
        for piece in &mut self.pieces {
            if matches!(piece, PieceStatus::Downloading { .. }) {
                *piece = PieceStatus::Missing;
            }
        }
    }

    pub fn has_incomplete_pieces(&self) -> bool {
        self.pieces
            .iter()
            .any(|p| !matches!(p, PieceStatus::Complete))
    }

    pub fn peer_has_incomplete_piece(&self, peer_bitfield: &[u8], total_pieces: usize) -> bool {
        let limit = total_pieces.min(self.pieces.len());
        (0..limit).any(|i| {
            !matches!(self.pieces[i], PieceStatus::Complete) && Self::check_piece(peer_bitfield, i)
        })
    }

    // mark PieceStatus::Complete when a piece is downloaded
    // verified againts SHA-1 Hash and written to disk..
    pub fn mark_complete(&mut self, piece_index: usize) {
        if piece_index < self.pieces.len() {
            self.pieces[piece_index] = PieceStatus::Complete
        }
    }

    // Called when a peer sends Have(index). Increments availability for that piece.
    pub fn register_have(&mut self, piece_index: usize) {
        if piece_index < self.piece_availability.len() {
            self.piece_availability[piece_index] =
                self.piece_availability[piece_index].saturating_add(1);
        }
    }

    pub fn all_complete(&self) -> bool {
        self.pieces
            .iter()
            .all(|p_s| matches!(p_s, PieceStatus::Complete))
    }

    pub fn to_bitfield(&self, total_pieces: usize) -> Vec<u8> {
        let num_bytes = (total_pieces + 7) / 8;
        let mut bitfield = vec![0u8; num_bytes];

        for i in 0..total_pieces {
            if matches!(self.pieces.get(i), Some(PieceStatus::Complete)) {
                let byte_idx = i / 8;
                let bit_idx = 7 - (i % 8);
                bitfield[byte_idx] |= 1 << bit_idx;
            }
        }

        bitfield
    }

    pub fn completed_pieces_count(&self) -> usize {
        self.pieces
            .iter()
            .filter(|p_s| matches!(p_s, PieceStatus::Complete))
            .count()
    }

    pub fn downloaded_bytes(&self, piece_length: u32, total_torrent_size: u64) -> u64 {
        let total_pieces = self.pieces.len();
        if total_pieces == 0 {
            return 0;
        }

        let mut downloaded: u64 = 0;
        for i in 0..total_pieces {
            if matches!(self.pieces[i], PieceStatus::Complete) {
                if i == total_pieces - 1 {
                    let last_piece_len = total_torrent_size % piece_length as u64;
                    if last_piece_len == 0 {
                        downloaded += piece_length as u64;
                    } else {
                        downloaded += last_piece_len;
                    }
                } else {
                    downloaded += piece_length as u64;
                }
            }
        }
        downloaded
    }

    pub fn remaining_bytes(&self, piece_length: u32, total_torrent_size: u64) -> u64 {
        let downloaded = self.downloaded_bytes(piece_length, total_torrent_size);
        total_torrent_size.saturating_sub(downloaded)
    }

    // checks a peer has a specific piece or not.
    pub fn check_piece(bitfield: &[u8], piece_index: usize) -> bool {
        let byte_index = piece_index / 8;
        let bit_offset = piece_index % 8;

        if byte_index >= bitfield.len() {
            return false;
        }

        // BitTorrent uses big-endian bit order
        // (most significant bit is index 0)
        // shifting bits & checking bitfield
        (bitfield[byte_index] & (1 << (7 - bit_offset))) != 0
    }

    pub fn from_hotstate_bitfield(bitfield: &[u8], total_pieces: usize) -> Self {
        let pieces = (0..total_pieces)
            .map(|i| {
                if Self::check_piece(bitfield, i) {
                    PieceStatus::Complete
                } else {
                    PieceStatus::Missing
                }
            })
            .collect();

        Self {
            pieces,
            piece_availability: vec![0u32; total_pieces],
            sequential: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_sequential_vs_rarest_allocation() {
        let total = 4;
        let mut alloc = PieceAllocator::new(total);
        // Piece 0 has availability 10
        // Piece 1 has availability 2
        // Piece 2 has availability 5
        // Piece 3 has availability 1
        alloc.piece_availability = vec![10, 2, 5, 1];

        let peer_bitfield = vec![0b11110000]; // peer has all 4 pieces
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        // Rarest-first: should pick piece 3 (availability 1)
        alloc.sequential = false;
        let chosen_rarest = alloc.allocate_piece(&peer_bitfield, &ip, total);
        assert_eq!(chosen_rarest, Some(3));
        alloc.release_piece(3);

        // Sequential: should pick piece 0 (lowest index first)
        alloc.sequential = true;
        let chosen_seq = alloc.allocate_piece(&peer_bitfield, &ip, total);
        assert_eq!(chosen_seq, Some(0));

        // Mark piece 0 complete, next should be piece 1
        alloc.mark_complete(0);
        let chosen_seq_next = alloc.allocate_piece(&peer_bitfield, &ip, total);
        assert_eq!(chosen_seq_next, Some(1));
    }

    #[test]
    fn test_downloaded_bytes_with_fractional_last_piece() {
        // 3 pieces, piece_length = 1MB (1,048,576), total_size = 2.5MB (2,621,440)
        let total_pieces = 3;
        let piece_length = 1_048_576u32;
        let total_size = 2_621_440u64; // Last piece is 524,288 bytes

        let mut alloc = PieceAllocator::new(total_pieces);
        assert_eq!(alloc.downloaded_bytes(piece_length, total_size), 0);
        assert_eq!(alloc.remaining_bytes(piece_length, total_size), total_size);

        // Complete piece 0
        alloc.mark_complete(0);
        assert_eq!(alloc.downloaded_bytes(piece_length, total_size), 1_048_576);

        // Complete piece 1
        alloc.mark_complete(1);
        assert_eq!(alloc.downloaded_bytes(piece_length, total_size), 2_097_152);

        // Complete piece 2 (last piece) -> should equal exactly total_size, NOT 3 * piece_length
        alloc.mark_complete(2);
        assert!(alloc.all_complete());
        assert_eq!(alloc.downloaded_bytes(piece_length, total_size), total_size);
        assert_eq!(alloc.remaining_bytes(piece_length, total_size), 0);
    }

    #[test]
    fn test_reset_in_progress_and_allocate_excluding() {
        let total = 2;
        let mut alloc = PieceAllocator::new(total);
        alloc.piece_availability = vec![5, 5];
        let peer_bitfield = vec![0b11000000];
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Complete piece 0, allocate piece 1 (last piece)
        alloc.mark_complete(0);
        let p = alloc.allocate_piece(&peer_bitfield, &ip, total);
        assert_eq!(p, Some(1));
        assert!(alloc.has_incomplete_pieces());
        assert!(alloc.peer_has_incomplete_piece(&peer_bitfield, total));

        // While piece 1 is Downloading, another allocation attempt returns None
        assert_eq!(alloc.allocate_piece(&peer_bitfield, &ip, total), None);

        // Simulate Pause/Shutdown resetting Downloading pieces back to Missing
        alloc.reset_in_progress();
        assert_eq!(alloc.pieces[1], PieceStatus::Missing);

        // Excluding piece 1 returns None when piece 1 is the only missing piece
        assert_eq!(
            alloc.allocate_piece_excluding(&peer_bitfield, &ip, total, Some(1)),
            None
        );
        // Another peer not excluding piece 1 can now allocate it
        assert_eq!(
            alloc.allocate_piece_excluding(&peer_bitfield, &ip, total, None),
            Some(1)
        );
    }
}
