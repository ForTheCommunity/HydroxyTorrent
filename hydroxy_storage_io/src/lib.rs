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
            dir_paths.push(TorrentDirPaths {
                dir_name,
                torrent_entries,
            });
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

    /// Returns all category subdirectories found under watch_dir up to max_depth,
    /// even if they contain no .torrent files.
    pub async fn scan_categories(
        watch_dir: &str,
        max_depth: usize,
    ) -> Result<Vec<String>, StorageIOError> {
        let watch_path = Path::new(watch_dir);
        let mut grouped_paths: HashMap<String, Vec<TorrentFileEntry>> = HashMap::new();
        Self::scan_paths_recursive(watch_path, "", 0, max_depth, &mut grouped_paths).await?;
        let mut categories: Vec<String> = grouped_paths
            .into_keys()
            .filter(|k| !k.is_empty())
            .collect();
        categories.sort();
        categories.dedup();
        Ok(categories)
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

                    grouped.entry(next_rel.clone()).or_default();

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

/// Moves a file or directory from `src` to `dst`.
/// First ensures the parent directory of `dst` exists.
/// Attempts `tokio::fs::rename(src, dst)`.
/// If `rename` fails (e.g. cross-device link EXDEV), falls back to recursive copy and remove.
pub async fn move_file_or_dir(src: &Path, dst: &Path) -> Result<(), StorageIOError> {
    if !src.exists() {
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }

    if fs::rename(src, dst).await.is_ok() {
        return Ok(());
    }

    // Fallback: Copy and remove
    if src.is_file() {
        fs::copy(src, dst).await?;
        let _ = fs::remove_file(src).await;
    } else if src.is_dir() {
        copy_dir_all(src, dst).await?;
        let _ = fs::remove_dir_all(src).await;
    }

    Ok(())
}

pub fn copy_dir_all<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageIOError>> + Send + 'a>> {
    Box::pin(async move {
        if !dst.exists() {
            fs::create_dir_all(dst).await?;
        }
        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let dest_path = dst.join(entry.file_name());
            if entry_path.is_dir() {
                copy_dir_all(&entry_path, &dest_path).await?;
            } else {
                fs::copy(&entry_path, &dest_path).await?;
            }
        }
        Ok(())
    })
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

    #[tokio::test]
    async fn test_move_file_or_dir_file_and_directory() {
        let temp_dir = std::env::temp_dir().join(format!("hydroxy_test_move_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
        let src_dir = temp_dir.join("src");
        let dst_dir = temp_dir.join("dst");

        // 1. Test single file move
        let file_src = src_dir.join("single.txt");
        let file_dst = dst_dir.join("single.txt");
        fs::create_dir_all(&src_dir).await.unwrap();
        fs::write(&file_src, b"hello world").await.unwrap();

        move_file_or_dir(&file_src, &file_dst).await.unwrap();
        assert!(!file_src.exists());
        assert!(file_dst.exists());
        let content = fs::read(&file_dst).await.unwrap();
        assert_eq!(content, b"hello world");

        // 2. Test directory move with nested files
        let folder_src = src_dir.join("my_torrent");
        let folder_dst = dst_dir.join("my_torrent");
        fs::create_dir_all(folder_src.join("subfolder")).await.unwrap();
        fs::write(folder_src.join("file1.dat"), b"data1").await.unwrap();
        fs::write(folder_src.join("subfolder/file2.dat"), b"data2").await.unwrap();

        move_file_or_dir(&folder_src, &folder_dst).await.unwrap();
        assert!(!folder_src.exists());
        assert!(folder_dst.exists());
        assert_eq!(fs::read(folder_dst.join("file1.dat")).await.unwrap(), b"data1");
        assert_eq!(fs::read(folder_dst.join("subfolder/file2.dat")).await.unwrap(), b"data2");

        let _ = fs::remove_dir_all(temp_dir).await;
    }
}
