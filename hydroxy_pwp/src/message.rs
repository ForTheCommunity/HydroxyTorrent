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
    Extended {
        id: u8,
        payload: Vec<u8>,
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

        match id {
            0 | 1 | 2 | 3 => {
                if payload_length != 0 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Unexpected payload for control message.".into(),
                    ));
                }
                match id {
                    0 => Ok(PeerMessage::Choke),
                    1 => Ok(PeerMessage::Unchoke),
                    2 => Ok(PeerMessage::Interested),
                    _ => Ok(PeerMessage::NotInterested),
                }
            }
            4 => {
                if payload_length != 4 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid Payload Length for 'Have'.".into(),
                    ));
                }
                let mut buf = [0u8; 4];
                tcp_stream
                    .read_exact(&mut buf)
                    .await
                    .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
                let index = u32::from_be_bytes(buf);
                Ok(PeerMessage::Have(index))
            }
            5 => {
                let mut payload_buffer = vec![0u8; payload_length];
                if payload_length > 0 {
                    tcp_stream
                        .read_exact(&mut payload_buffer)
                        .await
                        .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
                }
                Ok(PeerMessage::Bitfield(payload_buffer))
            }

            6 => {
                if payload_length != 12 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid Payload Length for Request".into(),
                    ));
                }

                let mut buf = [0u8; 12];
                tcp_stream
                    .read_exact(&mut buf)
                    .await
                    .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

                let index = u32::from_be_bytes(buf[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[4..8].try_into().unwrap());
                let length = u32::from_be_bytes(buf[8..12].try_into().unwrap());

                Ok(PeerMessage::Request {
                    index,
                    begin,
                    length,
                })
            }

            7 => {
                if payload_length < 8 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid payload length for Piece.".into(),
                    ));
                }

                let mut header = [0u8; 8];
                tcp_stream
                    .read_exact(&mut header)
                    .await
                    .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

                let index = u32::from_be_bytes(header[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(header[4..8].try_into().unwrap());

                let block_len = payload_length - 8;
                let mut block = vec![0u8; block_len];
                if block_len > 0 {
                    tcp_stream
                        .read_exact(&mut block)
                        .await
                        .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
                }

                Ok(PeerMessage::Piece {
                    index,
                    begin,
                    block,
                })
            }

            8 => {
                if payload_length != 12 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Invalid payload length for cancel.".into(),
                    ));
                }

                let mut buf = [0u8; 12];
                tcp_stream
                    .read_exact(&mut buf)
                    .await
                    .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

                let index = u32::from_be_bytes(buf[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[4..8].try_into().unwrap());
                let length = u32::from_be_bytes(buf[8..12].try_into().unwrap());

                Ok(PeerMessage::Cancel {
                    index,
                    begin,
                    length,
                })
            }

            20 => {
                if payload_length == 0 {
                    return Err(PeerWireProtocolError::InvalidPeerMessage(
                        "Extended message missing extended ID".into(),
                    ));
                }
                let mut ext_id_buf = [0u8; 1];
                tcp_stream
                    .read_exact(&mut ext_id_buf)
                    .await
                    .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
                let ext_id = ext_id_buf[0];

                let ext_payload_len = payload_length - 1;
                let mut ext_payload = vec![0u8; ext_payload_len];
                if ext_payload_len > 0 {
                    tcp_stream
                        .read_exact(&mut ext_payload)
                        .await
                        .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;
                }

                Ok(PeerMessage::Extended {
                    id: ext_id,
                    payload: ext_payload,
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
        let io_res = match self {
            PeerMessage::KeepAlive => tcp_stream.write_all(&0u32.to_be_bytes()).await,
            PeerMessage::Choke => tcp_stream.write_all(&[0, 0, 0, 1, 0]).await,
            PeerMessage::Unchoke => tcp_stream.write_all(&[0, 0, 0, 1, 1]).await,
            PeerMessage::Interested => tcp_stream.write_all(&[0, 0, 0, 1, 2]).await,
            PeerMessage::NotInterested => tcp_stream.write_all(&[0, 0, 0, 1, 3]).await,
            PeerMessage::Have(index) => {
                let mut buf = [0u8; 9];
                buf[0..4].copy_from_slice(&5u32.to_be_bytes());
                buf[4] = 4;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                tcp_stream.write_all(&buf).await
            }
            PeerMessage::Bitfield(bitfield) => {
                let len = (1 + bitfield.len()) as u32;
                let mut header = [0u8; 5];
                header[0..4].copy_from_slice(&len.to_be_bytes());
                header[4] = 5;
                tcp_stream.write_all(&header).await?;
                tcp_stream.write_all(bitfield).await
            }
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                let mut buf = [0u8; 17];
                buf[0..4].copy_from_slice(&13u32.to_be_bytes());
                buf[4] = 6;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                buf[9..13].copy_from_slice(&begin.to_be_bytes());
                buf[13..17].copy_from_slice(&length.to_be_bytes());
                tcp_stream.write_all(&buf).await
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                let len = (9 + block.len()) as u32;
                let mut header = [0u8; 13];
                header[0..4].copy_from_slice(&len.to_be_bytes());
                header[4] = 7;
                header[5..9].copy_from_slice(&index.to_be_bytes());
                header[9..13].copy_from_slice(&begin.to_be_bytes());
                tcp_stream.write_all(&header).await?;
                tcp_stream.write_all(block).await
            }
            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                let mut buf = [0u8; 17];
                buf[0..4].copy_from_slice(&13u32.to_be_bytes());
                buf[4] = 8;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                buf[9..13].copy_from_slice(&begin.to_be_bytes());
                buf[13..17].copy_from_slice(&length.to_be_bytes());
                tcp_stream.write_all(&buf).await
            }
            PeerMessage::Extended { id, payload } => {
                let len = (2 + payload.len()) as u32;
                let mut header = [0u8; 6];
                header[0..4].copy_from_slice(&len.to_be_bytes());
                header[4] = 20;
                header[5] = *id;
                tcp_stream.write_all(&header).await?;
                tcp_stream.write_all(payload).await
            }
        };
        io_res.map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_peer_message_write_read_roundtrip() {
        let messages = vec![
            PeerMessage::KeepAlive,
            PeerMessage::Choke,
            PeerMessage::Unchoke,
            PeerMessage::Interested,
            PeerMessage::NotInterested,
            PeerMessage::Have(42),
            PeerMessage::Bitfield(vec![0b1010_0101, 0b1100_0011]),
            PeerMessage::Request {
                index: 3,
                begin: 16384,
                length: 16384,
            },
            PeerMessage::Piece {
                index: 3,
                begin: 16384,
                block: vec![1, 2, 3, 4, 5, 6, 7, 8],
            },
            PeerMessage::Cancel {
                index: 3,
                begin: 16384,
                length: 16384,
            },
            PeerMessage::Extended {
                id: 1,
                payload: b"d1:md6:ut_pexi1eee".to_vec(),
            },
        ];

        for msg in messages {
            let mut buf = Vec::new();
            msg.write(&mut buf).await.unwrap();
            let mut cursor = Cursor::new(buf);
            let decoded = PeerMessage::read(&mut cursor).await.unwrap();
            assert_eq!(format!("{:?}", msg), format!("{:?}", decoded));
        }
    }
}
