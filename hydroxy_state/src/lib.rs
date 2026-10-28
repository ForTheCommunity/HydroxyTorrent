use std::collections::BTreeMap;

use hydroxy_bencode::{
    b_dict::BDict,
    decode::{BencodedValue, decode},
    encode::encode_ordered_dict,
};
use hydroxy_config::HydroxyConfig;
use hydroxy_utils::torrent_info::{OtherInfos, TorrentInfo};
use tracing::info;

use crate::errors::StateError;
pub mod errors;

pub const CURRENT_STATE_VERSION: i64 = 1;

#[derive(Clone, Debug)]
pub struct TorrentState {
    pub info_hash_v1: String,
    pub version: i64,
    pub added_time: i64,
    pub total_downloaded: u64,
    pub total_uploaded: u64,
    pub paused: i64,
    pub sequential: i64,
    pub pieces: Vec<u8>,
    pub trackers: Vec<String>,
}

impl From<TorrentState> for BencodedValue {
    fn from(state: TorrentState) -> Self {
        let mut map = BTreeMap::new();

        map.insert(
            b"info_hash_v1".to_vec(),
            BencodedValue::String(state.info_hash_v1.into_bytes()),
        );
        map.insert(
            b"version".to_vec(),
            BencodedValue::Integer(state.version),
        );
        map.insert(
            b"added_time".to_vec(),
            BencodedValue::Integer(state.added_time),
        );
        map.insert(
            b"total_downloaded".to_vec(),
            BencodedValue::Integer(state.total_downloaded as i64),
        );
        map.insert(
            b"total_uploaded".to_vec(),
            BencodedValue::Integer(state.total_uploaded as i64),
        );
        map.insert(b"paused".to_vec(), BencodedValue::Integer(state.paused));
        map.insert(
            b"sequential".to_vec(),
            BencodedValue::Integer(state.sequential),
        );
        map.insert(b"pieces".to_vec(), BencodedValue::String(state.pieces));

        let trackers: Vec<BencodedValue> = state
            .trackers
            .into_iter()
            .map(|t| BencodedValue::String(t.into_bytes()))
            .collect();
        map.insert(b"trackers".to_vec(), BencodedValue::List(trackers));

        BencodedValue::Dictionary(map)
    }
}

impl TorrentState {
    /// Encodes the state file in the exact schema order:
    /// 1. `info_hash_v1` (top)
    /// 2. `version`
    /// 3. `added_time`
    /// 4. `total_downloaded`
    /// 5. `total_uploaded`
    /// 6. `paused`
    /// 7. `sequential`
    /// 8. `pieces`
    /// 9. `trackers` (bottom)
    pub fn encode_state(&self) -> Vec<u8> {
        let trackers: Vec<BencodedValue> = self
            .trackers
            .iter()
            .map(|t| BencodedValue::String(t.as_bytes().to_vec()))
            .collect();

        let entries: [(&[u8], BencodedValue); 9] = [
            (
                b"info_hash_v1",
                BencodedValue::String(self.info_hash_v1.as_bytes().to_vec()),
            ),
            (b"version", BencodedValue::Integer(self.version)),
            (b"added_time", BencodedValue::Integer(self.added_time)),
            (
                b"total_downloaded",
                BencodedValue::Integer(self.total_downloaded as i64),
            ),
            (
                b"total_uploaded",
                BencodedValue::Integer(self.total_uploaded as i64),
            ),
            (b"paused", BencodedValue::Integer(self.paused)),
            (b"sequential", BencodedValue::Integer(self.sequential)),
            (b"pieces", BencodedValue::String(self.pieces.clone())),
            (b"trackers", BencodedValue::List(trackers)),
        ];

        encode_ordered_dict(&entries)
    }

    pub async fn load_state(
        info_hash_v1: &str,
        torrent_info: &TorrentInfo,
    ) -> Result<Self, StateError> {
        info!("Loading State...");
        let state_dir = HydroxyConfig::state_dir()?;
        let file_name = format!("{}.hot_state", info_hash_v1);

        let file_path = state_dir.join(file_name);

        if !file_path.exists() {
            info!("Creating State for : {}", &torrent_info.info.name);

            let total_pieces = torrent_info.info.pieces.len() / 20;
            let bitfield_len = (total_pieces + 7) / 8;
            Self::save_state(TorrentState {
                info_hash_v1: OtherInfos::to_sha1(&torrent_info.other_infos.info_hash),
                version: CURRENT_STATE_VERSION,
                added_time: chrono::Utc::now().timestamp(),
                total_downloaded: 0,
                total_uploaded: 0,
                paused: 0,
                sequential: 0,
                pieces: vec![0; bitfield_len],
                trackers: torrent_info.announce_list.clone(),
            })
            .await?;
        }

        let file = tokio::fs::read(file_path)
            .await
            .map_err(hydroxy_storage_io::errors::StorageIOError::from)?;

        Ok(Self::decode_state(file)?)
    }

    pub async fn save_state(state: Self) -> Result<(), StateError> {
        let file_name = format!("{}.hot_state", &state.info_hash_v1);
        let tmp_file_name = format!("{}.hot_state.tmp", &state.info_hash_v1);
        let encoded_data = state.encode_state();

        let state_dir = HydroxyConfig::state_dir()?;
        if !state_dir.exists() {
            tokio::fs::create_dir_all(&state_dir).await?;
        }
        let file_path = state_dir.join(file_name);
        let tmp_path = state_dir.join(tmp_file_name);

        // Write to a temporary file first and atomically rename over the target state file
        // so an unexpected exit or crash mid-write can never corrupt or truncate `.hot_state`.
        tokio::fs::write(&tmp_path, &encoded_data).await?;
        tokio::fs::rename(&tmp_path, &file_path).await?;
        Ok(())
    }

    pub fn mark_completed_piece(&mut self, piece_index: usize) {
        // index of byte which contains bit related to a piece.
        let byte_index = piece_index / 8;
        // index of bit inside that byte
        let bit_index = 7 - (piece_index % 8);

        if byte_index < self.pieces.len() {
            // flipping bit
            self.pieces[byte_index] |= 1 << bit_index;
        }
    }

    pub fn decode_state(file_bytes: Vec<u8>) -> Result<Self, StateError> {
        info!("Decoding State...");
        let decoded_file = decode(&file_bytes)?;

        let d = match decoded_file {
            BencodedValue::Dictionary(d) => d,
            _ => return Err(StateError::Error("invalid root".into())),
        };

        let dictionary = BDict::new(&d);

        // extracting data...
        let info_hash_v1 = dictionary
            .get_string(b"info_hash_v1")
            .or_else(|| dictionary.get_string(b"info_hash"))
            .and_then(|b| String::from_utf8(b.to_vec()).ok())
            .unwrap();
        let version = dictionary
            .get_int(b"version")
            .unwrap_or(CURRENT_STATE_VERSION);
        let added_time = dictionary.get_int(b"added_time").unwrap_or(0);
        let total_downloaded = dictionary
            .get_int(b"total_downloaded")
            .or_else(|| dictionary.get_int(b"downloaded"))
            .unwrap_or(0)
            .max(0) as u64;
        let total_uploaded = dictionary
            .get_int(b"total_uploaded")
            .or_else(|| dictionary.get_int(b"uploaded"))
            .unwrap_or(0)
            .max(0) as u64;
        let paused = dictionary.get_int(b"paused").unwrap_or(0);
        let sequential = dictionary.get_int(b"sequential").unwrap_or(0);
        let pieces: Vec<u8> = dictionary.get_string(b"pieces").unwrap().to_vec();

        let trackers = dictionary
            .get_list(b"trackers")
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| match v {
                BencodedValue::String(b) => String::from_utf8(b.clone()).ok(),
                _ => None,
            })
            .collect::<Vec<String>>();

        tracing::debug!(
            "DECODED TRACKERS FROM HOT FILE -> {:?} | v: {} | paused : {} | sequential : {} | info hash v1 {:?} | added time {} | dl: {} | ul: {}",
            trackers, version, paused, sequential, info_hash_v1, added_time, total_downloaded, total_uploaded
        );

        Ok(TorrentState {
            info_hash_v1,
            version,
            added_time,
            total_downloaded,
            total_uploaded,
            paused,
            sequential,
            pieces,
            trackers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_state_bencode_roundtrip_and_key_order() {
        let original = TorrentState {
            info_hash_v1: "abcd1234abcd1234abcd1234abcd1234abcd1234".to_string(),
            version: CURRENT_STATE_VERSION,
            added_time: 1729850000,
            total_downloaded: 10485760,
            total_uploaded: 5242880,
            paused: 1,
            sequential: 1,
            pieces: vec![0b10101010, 0b11110000],
            trackers: vec!["http://tracker.example.com/announce".to_string()],
        };

        let encoded_bytes = original.encode_state();
        // Verify info_hash_v1 is at the very top and trackers is at the bottom
        assert!(encoded_bytes.starts_with(b"d12:info_hash_v140:abcd1234abcd1234abcd1234abcd1234abcd1234"));
        assert!(encoded_bytes.ends_with(b"8:trackersl35:http://tracker.example.com/announceee"));

        let decoded = TorrentState::decode_state(encoded_bytes).expect("Failed to decode state");
        assert_eq!(decoded.info_hash_v1, original.info_hash_v1);
        assert_eq!(decoded.version, CURRENT_STATE_VERSION);
        assert_eq!(decoded.paused, 1);
        assert_eq!(decoded.sequential, 1);
        assert_eq!(decoded.pieces, original.pieces);
        assert_eq!(decoded.trackers, original.trackers);
        assert_eq!(decoded.total_downloaded, 10485760);
        assert_eq!(decoded.total_uploaded, 5242880);
    }
}
