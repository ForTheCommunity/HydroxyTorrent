use hydroxy_utils::torrent_info::TorrentInfo;

#[derive(Clone)]
pub struct FileRegion {
    // full path on disk
    pub path: String,
    // where this file starts in the flat stream
    pub offset_in_torrent: u64,
    pub length: u64,
}

impl FileRegion {
    pub fn build_file_map(torrent_info: &TorrentInfo, root_dir: &str) -> Vec<FileRegion> {
        // for single file torrent.
        if torrent_info.info.files.is_empty() {
            let length = torrent_info.info.length.unwrap_or(0) as u64;
            return vec![FileRegion {
                path: format!("{}/{}", root_dir, torrent_info.info.name),
                offset_in_torrent: 0,
                length,
            }];
        }

        // for multi file torrents...
        let mut offset = 0u64;
        torrent_info
            .info
            .files
            .iter()
            .map(|file| {
                let path = format!(
                    "{}/{}/{}",
                    root_dir,
                    torrent_info.info.name,
                    file.path.join("/")
                );

                let file_region = FileRegion {
                    path,
                    offset_in_torrent: offset,
                    length: file.length as u64,
                };
                offset += file.length as u64;
                file_region
            })
            .collect()
    }
}
