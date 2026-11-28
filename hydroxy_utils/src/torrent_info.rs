use hydroxy_bencode::{
    decode::{BencodedValue, decode},
    encode::encode,
};
use sha1::{Digest, Sha1};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub announce: String,
    pub announce_list: Vec<String>,
    pub comment: String,
    pub created_by: String,
    pub encoding: String,
    pub info: Info,
    pub url_list: Vec<String>,
    // Other Infos
    pub other_infos: OtherInfos,
}

impl TorrentInfo {
    pub fn is_private(&self) -> bool {
        self.info.is_private()
    }
}

#[derive(Debug, Clone)]
pub struct Info {
    // this is name of the torrent.
    pub name: String,
    // this is name encoded in UTF-8
    pub encoded_name: String,
    // for single-file torrents
    pub length: Option<usize>,
    // files/folders(dirs)
    pub files: Vec<File>,
    // total number of pieces.
    pub piece_length: usize,
    pub pieces: Vec<u8>,
    pub source: String,
    // BEP 0027: Private torrents
    pub private: Option<usize>,
}

impl Info {
    pub fn is_private(&self) -> bool {
        self.private == Some(1)
    }
}

#[derive(Debug, Clone)]
pub struct File {
    pub length: usize,
    pub path: Vec<String>,
    pub encoded_path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OtherInfos {
    pub info_hash: [u8; 20],
    pub total_pieces: usize,
}

#[derive(Debug, Error)]
pub enum TorrentParseError {
    #[error("Error Parsing Torrent File: {0}")]
    ParseError(String),
}

impl TorrentInfo {
    pub fn parse_torrent_file(file_data: Vec<u8>) -> Result<TorrentInfo, TorrentParseError> {
        let decoded_file_data =
            decode(&file_data).map_err(|e| TorrentParseError::ParseError(e.to_string()))?;

        let root_dictionary = match decoded_file_data {
            BencodedValue::Dictionary(dict) => dict,
            _ => {
                return Err(TorrentParseError::ParseError(
                    "Root is not a dictionary... Provide a Valid Torrent File".into(),
                ));
            }
        };

        let get_value = |key: &[u8]| -> String {
            match root_dictionary.get(key) {
                Some(BencodedValue::String(bytes)) => String::from_utf8_lossy(bytes).to_string(),
                _ => String::new(),
            }
        };

        // extracting torrent details
        let announce = get_value(b"announce");
        let comment = get_value(b"comment");
        let created_by = get_value(b"created by");
        let encoding = get_value(b"encoding");

        // extracting announce list..
        let mut announce_list: Vec<String> = Vec::new();
        if let Some(BencodedValue::List(list)) = root_dictionary.get(&b"announce-list"[..]) {
            for tier in list {
                if let BencodedValue::List(announce_urls) = tier {
                    for url in announce_urls {
                        if let BencodedValue::String(bytes) = url {
                            announce_list.push(String::from_utf8_lossy(bytes).to_string());
                        }
                    }
                }
            }
        }

        // info dictionary
        let bencoded_info_dict = match root_dictionary.get(&b"info"[..]) {
            Some(info_val @ BencodedValue::Dictionary(_)) => info_val,
            _ => {
                return Err(TorrentParseError::ParseError(
                    "Info Dictionary Missing !!!".into(),
                ));
            }
        };

        // Info Hash in Bittorrent Protocol is SHA-1 hash of bencoded info dictionary
        let info_bencoded = encode(bencoded_info_dict);
        let info_hash = Sha1::digest(&info_bencoded).into();

        // info dictionary
        let info_dict = if let BencodedValue::Dictionary(d) = bencoded_info_dict {
            d
        } else {
            // this else block is unreachable since bencoded_info_dict is a Dictionary.
            unreachable!()
        };

        // extracting name from info dictionary
        let name_bytes = match info_dict.get(&b"name"[..]) {
            Some(BencodedValue::String(bytes)) => bytes,
            _ => {
                return Err(TorrentParseError::ParseError(
                    "Name is Missing in Info Dictionary !!!".into(),
                ));
            }
        };
        let name = String::from_utf8_lossy(&name_bytes).to_string();

        // extracting encoded name from info dictionary
        let encoded_name_bytes: Vec<u8>;
        match info_dict.get(&b"name.utf-8"[..]) {
            Some(BencodedValue::String(bytes)) => encoded_name_bytes = bytes.clone(),
            _ => {
                encoded_name_bytes = "N/A".bytes().collect();
            }
        }

        let encoded_name = String::from_utf8_lossy(&encoded_name_bytes).to_string();

        // extracting piece length from info dictionary
        let piece_length = match info_dict.get(&b"piece length"[..]) {
            Some(BencodedValue::Integer(value)) => *value as usize,
            _ => {
                return Err(TorrentParseError::ParseError(
                    "Piece Length is Missing in Info Dictionary !!!".into(),
                ));
            }
        };

        // extracting pieces from info dictionary
        let pieces = match info_dict.get(&b"pieces"[..]) {
            Some(BencodedValue::String(bytes)) => bytes.to_owned(),
            _ => {
                return Err(TorrentParseError::ParseError(
                    "Pieces are Missing in Info Dictionary !!!".into(),
                ));
            }
        };

        // check each SHA1 Hash is 20 bytes or not.
        if pieces.len() % 20 != 0 {
            return Err(TorrentParseError::ParseError(
                "Invalid pieces field length".into(),
            ));
        }

        // total pieces
        let total_pieces = pieces.len() / 20;

        // extracting 'length' key that is used for single single file torrent.
        let length = match info_dict.get(&b"length"[..]) {
            Some(BencodedValue::Integer(value)) => Some(*value as usize),
            _ => None,
        };

        // extracting all Files from info dictionary
        let mut files: Vec<File> = Vec::new();

        if let Some(BencodedValue::List(files_list)) = info_dict.get(&b"files"[..]) {
            // extracting details of each file.
            for a_file in files_list {
                if let BencodedValue::Dictionary(file_dict) = a_file {
                    // extracting file length
                    let length = match file_dict.get(&b"length"[..]) {
                        Some(BencodedValue::Integer(val)) => *val as usize,
                        _ => continue,
                    };

                    // extracting path
                    let mut path = Vec::new();
                    if let Some(BencodedValue::List(path_list)) = file_dict.get(&b"path"[..]) {
                        // path_list means list of sub directories.
                        for a_path in path_list {
                            if let BencodedValue::String(path_bytes) = a_path {
                                path.push(String::from_utf8_lossy(path_bytes).to_string());
                            }
                        }
                    }

                    // extracting Encoded Path
                    let mut encoded_path = Vec::new();
                    if let Some(BencodedValue::List(encoded_path_list)) =
                        file_dict.get(&b"path.utf-8"[..])
                    {
                        for a_path in encoded_path_list {
                            if let BencodedValue::String(enc_path_bytes) = a_path {
                                encoded_path
                                    .push(String::from_utf8_lossy(enc_path_bytes).to_string());
                            }
                        }
                    }
                    // if encoded path is Empty
                    if encoded_path.is_empty() {
                        encoded_path = path.clone();
                    }

                    files.push(File {
                        length,
                        path,
                        encoded_path,
                    });
                };
            }
        }

        // extracting source field
        let source = match info_dict.get(&b"source"[..]) {
            Some(BencodedValue::String(s)) => String::from_utf8_lossy(s).to_string(),
            _ => "N/A".into(),
        };

        // extracting private field (BEP 0027)
        let private = match info_dict.get(&b"private"[..]) {
            Some(BencodedValue::Integer(val)) => Some(*val as usize),
            _ => None,
        };

        // extracting url-list (BEP 0019: single string, list of strings, or nested tiered lists)
        let mut url_list: Vec<String> = Vec::new();
        let mut push_clean_url = |bytes: &[u8]| {
            let trimmed = String::from_utf8_lossy(bytes).trim().to_string();
            if !trimmed.is_empty() && !url_list.contains(&trimmed) {
                url_list.push(trimmed);
            }
        };
        match root_dictionary.get(&b"url-list"[..]) {
            Some(BencodedValue::List(list)) => {
                for item in list {
                    match item {
                        BencodedValue::String(bytes) => push_clean_url(bytes),
                        BencodedValue::List(tier) => {
                            for nested in tier {
                                if let BencodedValue::String(bytes) = nested {
                                    push_clean_url(bytes);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some(BencodedValue::String(bytes)) => push_clean_url(bytes),
            _ => {}
        }

        Ok(TorrentInfo {
            announce,
            announce_list,
            comment,
            created_by,
            encoding,
            info: Info {
                name,
                encoded_name,
                length,
                files,
                piece_length,
                pieces,
                source,
                private,
            },
            url_list,
            other_infos: OtherInfos {
                info_hash,
                total_pieces,
            },
        })
    }
}

impl OtherInfos {
    pub fn to_sha1(raw_ih: &[u8; 20]) -> String {
        raw_ih
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_info_dict() -> BencodedValue {
        let mut info = BTreeMap::new();
        info.insert(b"length".to_vec(), BencodedValue::Integer(16));
        info.insert(
            b"name".to_vec(),
            BencodedValue::String(b"ubuntu.iso".to_vec()),
        );
        info.insert(b"piece length".to_vec(), BencodedValue::Integer(16));
        info.insert(b"pieces".to_vec(), BencodedValue::String(vec![0xAA; 20]));
        BencodedValue::Dictionary(info)
    }

    #[test]
    fn test_bep0019_url_list_single_string_and_list_variants() {
        // 1. Single string `url-list` (GetRight / mktorrent single-webseed convention)
        let mut root1 = BTreeMap::new();
        root1.insert(b"info".to_vec(), sample_info_dict());
        root1.insert(
            b"url-list".to_vec(),
            BencodedValue::String(b"  https://releases.ubuntu.com/24.04/ubuntu.iso  ".to_vec()),
        );
        let parsed1 =
            TorrentInfo::parse_torrent_file(encode(&BencodedValue::Dictionary(root1))).unwrap();
        assert_eq!(
            parsed1.url_list,
            vec!["https://releases.ubuntu.com/24.04/ubuntu.iso"]
        );

        // 2. List + deduplication + nested list `url-list`
        let mut root2 = BTreeMap::new();
        root2.insert(b"info".to_vec(), sample_info_dict());
        root2.insert(
            b"url-list".to_vec(),
            BencodedValue::List(vec![
                BencodedValue::String(b"https://mirror1.example.com/pub/".to_vec()),
                BencodedValue::String(b"".to_vec()),
                BencodedValue::String(b"https://mirror1.example.com/pub/".to_vec()),
                BencodedValue::List(vec![BencodedValue::String(
                    b"http://mirror2.example.com/pub/".to_vec(),
                )]),
            ]),
        );
        let parsed2 =
            TorrentInfo::parse_torrent_file(encode(&BencodedValue::Dictionary(root2))).unwrap();
        assert_eq!(
            parsed2.url_list,
            vec![
                "https://mirror1.example.com/pub/",
                "http://mirror2.example.com/pub/"
            ]
        );
    }
}
