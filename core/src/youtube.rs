use std::{fs, path::{Path, PathBuf}, time::Duration};

use yt_dlp::{
    download::config::postprocess::{AudioCodec, PostProcessConfig},
    Downloader,
};

pub struct VideoInfo {
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<Duration>,
}

pub fn fetch_info(url: &str, binaries_dir: &Path) -> Result<VideoInfo, Error> {
    run(async {
        let downloader = build_downloader(binaries_dir).await?;
        let video = downloader.fetch_video_infos(url).await.map_err(Error::YtDlp)?;

        if video.is_live == Some(true) {
            return Err(Error::Live);
        }

        Ok(VideoInfo {
            title: video.title,
            uploader: video.uploader,
            duration: video.duration.and_then(|secs| u64::try_from(secs).ok()).map(Duration::from_secs),
        })
    })
}

pub fn download_audio(url: &str, binaries_dir: &Path, dest_path: &Path) -> Result<(), Error> {
    run(async {
        let downloader = build_downloader(binaries_dir).await?;
        let video = downloader.fetch_video_infos(url).await.map_err(Error::YtDlp)?;

        if video.is_live == Some(true) {
            return Err(Error::Live);
        }

        let dir = dest_path.parent().ok_or_else(|| Error::InvalidDestination(dest_path.to_path_buf()))?;
        let temp = tempfile::Builder::new()
            .prefix(".yt-dlp-download-")
            .suffix(".tmp")
            .tempfile_in(dir)
            .map_err(Error::Temp)?;
        let temp_path = temp.path().to_path_buf();
        temp.close().map_err(Error::Temp)?;

        let download_result =
            downloader.download_audio_stream_to_path(&video, &temp_path).await.map_err(Error::YtDlp);

        let downloaded_path = match download_result {
            Ok(path) => path,
            Err(e) => {
                let _ = fs::remove_file(&temp_path);
                return Err(e);
            }
        };

        let config = PostProcessConfig::new().with_audio_codec(AudioCodec::MP3);
        let postprocess_result =
            downloader.postprocess_video_to_path(&downloaded_path, dest_path, config).await.map_err(Error::YtDlp);

        if let Err(e) = fs::remove_file(&downloaded_path) {
            eprintln!("warning: failed to remove temporary download {}: {e}", downloaded_path.display());
        }

        postprocess_result.map(|_| ())
    })
}

async fn build_downloader(binaries_dir: &Path) -> Result<Downloader, Error> {
    Downloader::with_new_binaries(binaries_dir, binaries_dir)
        .await
        .map_err(Error::YtDlp)?
        .build()
        .await
        .map_err(Error::YtDlp)
}

fn run<F, T>(future: F) -> Result<T, Error>
where
    F: std::future::Future<Output = Result<T, Error>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Runtime)?;
    runtime.block_on(future)
}

pub fn generate_file_name(artist: &str, title: &str) -> String {
    format!("{}-{}.mp3", generate_name_chunk(artist), generate_name_chunk(title))
}

fn generate_name_chunk(field: &str) -> String {
    field
        .split(|c: char| c.is_whitespace() || c == '-')
        .map(capitalize_and_strip)
        .collect::<Vec<_>>()
        .concat()
}

fn capitalize_and_strip(word: &str) -> String {
    let mut chars = word.chars();
    let capitalized = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    capitalized.chars().filter(|c| c.is_alphanumeric()).collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("this video is a live stream and cannot be downloaded")]
    Live,
    #[error("invalid destination path: {}", .0.display())]
    InvalidDestination(PathBuf),
    #[error("failed to create a temporary file for the download")]
    Temp(#[source] std::io::Error),
    #[error("failed to start the download runtime")]
    Runtime(#[source] std::io::Error),
    #[error(transparent)]
    YtDlp(#[from] yt_dlp::error::Error),
}
