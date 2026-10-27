use std::collections::BTreeMap;

use hydroxy_bencode::{
    b_dict::BDict,
    decode::{BencodedValue, decode},
    encode::encode,
};
use hydroxy_config::HydroxyConfig;
use hydroxy_utils::torrent_info::{OtherInfos, TorrentInfo};
use tracing::info;

use crate::errors::StateError;
pub mod errors;

#[derive(Clone)]
pub struct TorrentState {
    pub info_hash: String,
    pub trackers: Vec<String>,
    pub added_time: i64,
    pub paused: i64,
    pub sequential: i64,
    pub category: String,
    pub pieces: Vec<u8>,
}

impl From<TorrentState> for BencodedValue {
    fn from(state: TorrentState) -> Self {
        let mut map = BTreeMap::new();

        map.insert(
            b"info_hash".to_vec(),
            BencodedValue::String(state.info_hash.into_bytes()),
        );
        map.insert(
            b"added_time".to_vec(),
            BencodedValue::Integer(state.added_time),
        );

        let trackers: Vec<BencodedValue> = state
            .trackers
            .into_iter()
            .map(|t| BencodedValue::String(t.into_bytes()))
            .collect();
        map.insert(b"trackers".to_vec(), BencodedValue::List(trackers));

        map.insert(b"paused".to_vec(), BencodedValue::Integer(state.paused));
        map.insert(b"sequential".to_vec(), BencodedValue::Integer(state.sequential));
        map.insert(
            b"category".to_vec(),
            BencodedValue::String(state.category.into_bytes()),
        );
        map.insert(b"pieces".to_vec(), BencodedValue::String(state.pieces));

        BencodedValue::Dictionary(map)
    }
}

impl TorrentState {
    pub async fn load_state(
        info_hash: &str,
        torrent_info: &TorrentInfo,
    ) -> Result<Self, StateError> {
        info!("Loading State...");
        let state_dir = HydroxyConfig::state_dir()?;
        let file_name = format!("{}.hot_state", info_hash);

        let file_path = state_dir.join(file_name);

        if !file_path.exists() {
            info!("Creating State for : {}", &torrent_info.info.name);

            let total_pieces = torrent_info.info.pieces.len() / 20;
            let bitfield_len = (total_pieces + 7) / 8;
            Self::save_state(TorrentState {
                info_hash: OtherInfos::to_sha1(&torrent_info.other_infos.info_hash),
                trackers: torrent_info.announce_list.clone(),
                added_time: chrono::Utc::now().timestamp(),
                paused: 0,
                sequential: 0,
                category: String::new(),
                pieces: vec![0; bitfield_len],
            })
            .await?;
        }

        let file = std::fs::read(file_path).map_err(hydroxy_storage_io::errors::StorageIOError::from)?;

        Ok(Self::decode_state(file)?)
    }

    pub async fn save_state(state: Self) -> Result<(), StateError> {
        let file_name = format!("{}.hot_state", &state.info_hash);
        let tmp_file_name = format!("{}.hot_state.tmp", &state.info_hash);
        let bencoded_data: BencodedValue = state.into();
        let encoded_data = encode(&bencoded_data);

        let state_dir = HydroxyConfig::state_dir()?;
        if !state_dir.exists() {
            std::fs::create_dir_all(&state_dir)?;
        }
        let file_path = state_dir.join(file_name);
        let tmp_path = state_dir.join(tmp_file_name);

        // Write to a temporary file first and atomically rename over the target state file
        // so an unexpected exit or crash mid-write can never corrupt or truncate `.hot_state`.
        std::fs::write(&tmp_path, &encoded_data)?;
        std::fs::rename(&tmp_path, &file_path)?;
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

    fn decode_state(file_bytes: Vec<u8>) -> Result<Self, StateError> {
        info!("Decoding State...");
        let decoded_file = decode(&file_bytes)?;

        let d = match decoded_file {
            BencodedValue::Dictionary(d) => d,
            _ => return Err(StateError::Error("invalid root".into())),
        };

        let dictionary = BDict::new(&d);

        // extracting data...
        let added_time = dictionary.get_int(b"added_time").unwrap_or(0);
        let info_hash = dictionary
            .get_string(b"info_hash")
            .and_then(|b| String::from_utf8(b.to_vec()).ok())
            .unwrap();
        let paused = dictionary.get_int(b"paused").unwrap_or(0);
        let sequential = dictionary.get_int(b"sequential").unwrap_or(0);
        let category = dictionary
            .get_string(b"category")
            .and_then(|b| String::from_utf8(b.to_vec()).ok())
            .unwrap_or_default();
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

        println!(
            "DECODED TRACKERS FROM HOT FILE -> {:?} | paused : {} | sequential : {} | category : {:?} | info hash {:?} | added time {}",
            trackers, paused, sequential, category, info_hash, added_time
        );

        Ok(TorrentState {
            info_hash,
            trackers,
            added_time,
            paused,
            sequential,
            category,
            pieces: pieces,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrent_state_bencode_roundtrip() {
        let original = TorrentState {
            info_hash: "abcd1234abcd1234abcd1234abcd1234abcd1234".to_string(),
            trackers: vec!["http://tracker.example.com/announce".to_string()],
            added_time: 1729850000,
            paused: 1,
            sequential: 1,
            category: "anime/bleach".to_string(),
            pieces: vec![0b10101010, 0b11110000],
        };

        let bencoded: BencodedValue = original.clone().into();
        let encoded_bytes = encode(&bencoded);

        let decoded = TorrentState::decode_state(encoded_bytes).expect("Failed to decode state");
        assert_eq!(decoded.info_hash, original.info_hash);
        assert_eq!(decoded.paused, 1);
        assert_eq!(decoded.sequential, 1);
        assert_eq!(decoded.category, "anime/bleach");
        assert_eq!(decoded.pieces, original.pieces);
        assert_eq!(decoded.trackers, original.trackers);
    }
}
