use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::errors::PeerWireProtocolError;

#[derive(Debug, Clone)]
pub enum PeerMessage {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
}

impl PeerMessage {
    pub async fn read<R: AsyncRead + Unpin>(
        tcp_stream: &mut R,
    ) -> Result<Self, PeerWireProtocolError> {
        // Reading length prefix (4 Bytes)
        let mut length_buffer = [0u8; 4];
        tcp_stream
            .read_exact(&mut length_buffer)
            .await
            .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
        // length is encoded in Big Endian.
        let length = u32::from_be_bytes(length_buffer);

        // if KeepAlive Message has length 0 & no ID & no payload.
        if length == 0 {
            return Ok(PeerMessage::KeepAlive);
        }

        // Maximum message length limit (2MB) to prevent malformed or malicious
        // length prefixes from causing huge memory allocations. Standard blocks are 16KB.
        if length > 2 * 1024 * 1024 {
            return Err(PeerWireProtocolError::InvalidPeerMessage(format!(
                "Message length {} exceeds maximum allowed limit of 2MB",
                length
            )));
        }

        // Read message ID(1 Byte)
        let mut id_buffer = [0u8; 1];
        tcp_stream
            .read_exact(&mut id_buffer)
            .await
            .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
        let id = id_buffer[0];

        // Reading payload (length - 1 Byte for ID)
        let payload_length = (length - 1) as usize;
        let mut payload_buffer = vec![0u8; payload_length];
        if payload_length > 0 {
            tcp_stream
                .read_exact(&mut payload_buffer)
                .await
                .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
        }

        match id {
            0 => Ok(PeerMessage::Choke),
            1 => Ok(PeerMessage::Unchoke),
            2 => Ok(PeerMessage::Interested),
            3 => Ok(PeerMessage::NotInterested),
            4 => {
                if payload_length != 4 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid Payload Length for 'Have'.".into(),
                    ));
                }
                let index = u32::from_be_bytes(payload_buffer[0..4].try_into().unwrap());
                Ok(PeerMessage::Have(index))
            }
            5 => Ok(PeerMessage::Bitfield(payload_buffer)),

            6 => {
                if payload_buffer.len() != 12 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid Payload Length for Request".into(),
                    ));
                }

                let index = u32::from_be_bytes(payload_buffer[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(payload_buffer[4..8].try_into().unwrap());
                let length = u32::from_be_bytes(payload_buffer[8..12].try_into().unwrap());

                Ok(PeerMessage::Request {
                    index,
                    begin,
                    length,
                })
            }

            7 => {
                if payload_buffer.len() < 8 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid payload length for Piece.".into(),
                    ));
                }

                let index = u32::from_be_bytes(payload_buffer[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(payload_buffer[4..8].try_into().unwrap());

                let mut block = payload_buffer;
                block.drain(0..8);

                Ok(PeerMessage::Piece {
                    index,
                    begin,
                    block,
                })
            }

            8 => {
                if payload_buffer.len() != 12 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid payload length for cancel.".into(),
                    ));
                }

                let index = u32::from_be_bytes(payload_buffer[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(payload_buffer[4..8].try_into().unwrap());
                let length = u32::from_be_bytes(payload_buffer[8..12].try_into().unwrap());

                Ok(PeerMessage::Cancel {
                    index,
                    begin,
                    length,
                })
            }

            _ => Err(PeerWireProtocolError::InvalidPeerMessage(format!(
                "Unknown Message ID: {}",
                id
            ))),
        }
    }

    pub async fn write<W: AsyncWrite + Unpin>(
        &self,
        tcp_stream: &mut W,
    ) -> Result<(), PeerWireProtocolError> {
        let (id, payload): (Option<u8>, Vec<u8>) = match self {
            PeerMessage::KeepAlive => (None, vec![]),
            PeerMessage::Choke => (Some(0), vec![]),
            PeerMessage::Unchoke => (Some(1), vec![]),
            PeerMessage::Interested => (Some(2), vec![]),
            PeerMessage::NotInterested => (Some(3), vec![]),
            PeerMessage::Have(index) => (Some(4), index.to_be_bytes().to_vec()),
            PeerMessage::Bitfield(bitfield) => (Some(5), bitfield.clone()),
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                let mut p = Vec::with_capacity(12);
                p.extend_from_slice(&index.to_be_bytes());
                p.extend_from_slice(&begin.to_be_bytes());
                p.extend_from_slice(&length.to_be_bytes());
                (Some(6), p)
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                let mut p = Vec::with_capacity(8 + block.len());
                p.extend_from_slice(&index.to_be_bytes());
                p.extend_from_slice(&begin.to_be_bytes());
                p.extend_from_slice(block);
                (Some(7), p)
            }
            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                let mut p = Vec::with_capacity(12);
                p.extend_from_slice(&index.to_be_bytes());
                p.extend_from_slice(&begin.to_be_bytes());
                p.extend_from_slice(&length.to_be_bytes());
                (Some(8), p)
            }
        };

        match id {
            // KeepAlive: length = 0, no id, no payload
            None => tcp_stream.write_all(&0u32.to_be_bytes()).await,
            Some(id_byte) => {
                // length prefix = 1 (id) + payload
                let length = (1 + payload.len()) as u32;
                tcp_stream.write_all(&length.to_be_bytes()).await?;
                tcp_stream.write_all(&[id_byte]).await?;
                if !payload.is_empty() {
                    tcp_stream.write_all(&payload).await?;
                }
                Ok(())
            }
        }
        .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

        // Flush to ensure all bytes are actually sent to the peer.
        // Without this, pipelined Request messages can sit in the buffer
        // and the peer never sees them.
        tcp_stream
            .flush()
            .await
            .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

        Ok(())
    }

    // checks a peer has a specific piece or not.
    pub fn check_piece(bitfield: &[u8], piece_index: usize) -> bool {
        let byte_index = piece_index / 8;
        let bit_offset = piece_index % 8;

        if byte_index >= bitfield.len() {
            return false;
        }

        // BitTorrent uses big-endian bit order (most significant bit is index 0)
        // shifting bits & checking bitfield
        (bitfield[byte_index] & (1 << (7 - bit_offset))) != 0
    }
}
