use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::errors::StorageIOError;
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};
use tracing::{info, warn};

pub mod errors;
pub mod file_region;
pub mod piece_io;

pub struct TorrentDirData {
    pub dir_name: String,
    pub torrent_files: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct TorrentFileEntry {
    pub path: PathBuf,
    pub modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone)]
pub struct TorrentDirPaths {
    pub dir_name: String,
    pub torrent_entries: Vec<TorrentFileEntry>,
}

impl TorrentDirData {
    /// Scans the watch directory for `.torrent` file paths and modification timestamps
    /// without reading file contents into memory upfront.
    pub async fn scan_paths(
        watch_dir: &str,
        download_dir: &str,
        max_depth: usize,
    ) -> Result<Vec<TorrentDirPaths>, StorageIOError> {
        let watch_path = Path::new(watch_dir);
        let mut grouped_paths: HashMap<String, Vec<TorrentFileEntry>> = HashMap::new();

        Self::scan_paths_recursive(watch_path, "", 0, max_depth, &mut grouped_paths).await?;

        let mut dir_paths: Vec<TorrentDirPaths> = Vec::new();
        for (dir_name, torrent_entries) in grouped_paths {
            if !torrent_entries.is_empty() {
                dir_paths.push(TorrentDirPaths {
                    dir_name,
                    torrent_entries,
                });
            }
        }

        // dir reflection: ensure download directories mirror watch directory structure
        for torrent_dir in &dir_paths {
            if !torrent_dir.dir_name.is_empty() {
                let dir_path = Path::new(download_dir).join(&torrent_dir.dir_name);
                if !dir_path.exists() {
                    fs::create_dir_all(&dir_path).await?;
                }
            }
        }

        Ok(dir_paths)
    }

    fn scan_paths_recursive<'a>(
        current_dir: &'a Path,
        rel_dir: &'a str,
        current_depth: usize,
        max_depth: usize,
        grouped: &'a mut HashMap<String, Vec<TorrentFileEntry>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageIOError>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = match fs::read_dir(current_dir).await {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory {:?}: {}", current_dir, e);
                    return Ok(());
                }
            };

            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                if file_name.starts_with('.') {
                    continue;
                }

                if entry_path.is_dir() {
                    let next_depth = current_depth + 1;
                    let next_rel = if current_depth >= max_depth {
                        rel_dir.to_string()
                    } else if rel_dir.is_empty() {
                        file_name
                    } else {
                        format!("{}/{}", rel_dir, file_name)
                    };

                    Self::scan_paths_recursive(
                        &entry_path,
                        &next_rel,
                        next_depth,
                        max_depth,
                        grouped,
                    )
                    .await?;
                } else if entry_path.is_file()
                    && entry_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("torrent"))
                        .unwrap_or(false)
                {
                    let modified = entry.metadata().await.ok().and_then(|m| m.modified().ok());
                    grouped
                        .entry(rel_dir.to_string())
                        .or_default()
                        .push(TorrentFileEntry {
                            path: entry_path,
                            modified,
                        });
                }
            }

            Ok(())
        })
    }

    pub async fn init(
        watch_dir: &str,
        download_dir: &str,
        max_depth: usize,
    ) -> Result<Vec<TorrentDirData>, StorageIOError> {
        let watch_path = Path::new(watch_dir);
        let mut grouped_torrents: HashMap<String, Vec<Vec<u8>>> = HashMap::new();

        Self::scan_dir_recursive(watch_path, "", 0, max_depth, &mut grouped_torrents).await?;

        let mut torrents_dir_data: Vec<TorrentDirData> = Vec::new();
        for (dir_name, torrent_files) in grouped_torrents {
            if !torrent_files.is_empty() {
                torrents_dir_data.push(TorrentDirData {
                    dir_name,
                    torrent_files,
                });
            }
        }

        // dir reflection: ensure download directories mirror watch directory structure
        for torrent_dir in &torrents_dir_data {
            if !torrent_dir.dir_name.is_empty() {
                let dir_path = Path::new(download_dir).join(&torrent_dir.dir_name);
                if !dir_path.exists() {
                    fs::create_dir_all(&dir_path).await?;
                }
            }
        }

        Ok(torrents_dir_data)
    }

    fn scan_dir_recursive<'a>(
        current_dir: &'a Path,
        rel_dir: &'a str,
        current_depth: usize,
        max_depth: usize,
        grouped: &'a mut HashMap<String, Vec<Vec<u8>>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageIOError>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = match fs::read_dir(current_dir).await {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory {:?}: {}", current_dir, e);
                    return Ok(());
                }
            };

            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files/directories (e.g. .git, .DS_Store)
                if file_name.starts_with('.') {
                    continue;
                }

                if entry_path.is_dir() {
                    let next_depth = current_depth + 1;
                    let next_rel = if current_depth >= max_depth {
                        // Clamp at max_depth: keep current rel_dir for deeper subfolders
                        rel_dir.to_string()
                    } else if rel_dir.is_empty() {
                        file_name
                    } else {
                        format!("{}/{}", rel_dir, file_name)
                    };

                    Self::scan_dir_recursive(
                        &entry_path,
                        &next_rel,
                        next_depth,
                        max_depth,
                        grouped,
                    )
                    .await?;
                } else if entry_path.is_file()
                    && entry_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("torrent"))
                        .unwrap_or(false)
                {
                    if let Ok(mut file) = File::open(&entry_path).await {
                        let mut buffer = Vec::new();
                        if file.read_to_end(&mut buffer).await.is_ok() {
                            grouped
                                .entry(rel_dir.to_string())
                                .or_default()
                                .push(buffer);
                        }
                    }
                }
            }

            Ok(())
        })
    }
}

pub async fn read_file_to_bytes(file_path: PathBuf) -> Result<Vec<u8>, StorageIOError> {
    warn!("FP -> {}", file_path.display());

    if file_path.exists() {
        info!("File does exists....");
    }

    let mut file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_torrent_dir_data_recursive_and_depth_clamping() {
        let temp_dir = std::env::temp_dir().join(format!("hydroxy_test_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
        let watch_dir = temp_dir.join("watch");
        let download_dir = temp_dir.join("download");

        // Create directory structure:
        // watch/
        //   root.torrent
        //   anime/
        //     bleach/
        //       bleach_ep1.torrent
        //       season_1/
        //         deep_extra/
        //           extra.torrent
        //   movies/
        //     inception.torrent

        fs::create_dir_all(&watch_dir).await.unwrap();
        fs::create_dir_all(watch_dir.join("anime/bleach/season_1/deep_extra")).await.unwrap();
        fs::create_dir_all(watch_dir.join("movies")).await.unwrap();

        fs::write(watch_dir.join("root.torrent"), b"dummy root torrent").await.unwrap();
        fs::write(watch_dir.join("anime/bleach/bleach_ep1.torrent"), b"dummy bleach").await.unwrap();
        fs::write(watch_dir.join("anime/bleach/season_1/deep_extra/extra.torrent"), b"dummy extra").await.unwrap();
        fs::write(watch_dir.join("movies/inception.torrent"), b"dummy movie").await.unwrap();

        // Scan with max_depth = 2
        let results = TorrentDirData::init(
            watch_dir.to_str().unwrap(),
            download_dir.to_str().unwrap(),
            2,
        )
        .await
        .unwrap();

        let mut map: HashMap<String, usize> = HashMap::new();
        for r in results {
            map.insert(r.dir_name, r.torrent_files.len());
        }

        assert_eq!(map.get(""), Some(&1)); // root.torrent
        assert_eq!(map.get("movies"), Some(&1)); // movies/inception.torrent
        // bleach_ep1 and extra.torrent (clamped at depth 2) should both be under anime/bleach
        assert_eq!(map.get("anime/bleach"), Some(&2));

        // Verify directory reflection in download_dir
        assert!(download_dir.join("movies").exists());
        assert!(download_dir.join("anime/bleach").exists());

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir).await;
    }
}
