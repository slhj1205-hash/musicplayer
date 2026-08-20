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

pub fn fetch_and_download(
    url: &str,
    binaries_dir: &Path,
    scratch_dir: &Path,
    on_info: impl FnOnce(VideoInfo),
) -> Result<PathBuf, Error> {
    run(async {
        let downloader = build_downloader(binaries_dir).await?;
        let video = downloader.fetch_video_infos(url).await.map_err(Error::YtDlp)?;

        if video.is_live == Some(true) {
            return Err(Error::Live);
        }

        on_info(VideoInfo {
            title: video.title.clone(),
            uploader: video.uploader.clone(),
            duration: video.duration.and_then(|secs| u64::try_from(secs).ok()).map(Duration::from_secs),
        });

        let temp = tempfile::Builder::new()
            .prefix(".yt-dlp-download-")
            .suffix(".tmp")
            .tempfile_in(scratch_dir)
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

        let audio_temp = tempfile::Builder::new()
            .prefix(".yt-dlp-audio-")
            .suffix(".mp3")
            .tempfile_in(scratch_dir)
            .map_err(Error::Temp)?;
        let audio_path = audio_temp.path().to_path_buf();
        audio_temp.close().map_err(Error::Temp)?;

        let config = PostProcessConfig::new().with_audio_codec(AudioCodec::MP3);
        let postprocess_result =
            downloader.postprocess_video_to_path(&downloaded_path, &audio_path, config).await.map_err(Error::YtDlp);

        if let Err(e) = fs::remove_file(&downloaded_path) {
            eprintln!("warning: failed to remove temporary download {}: {e}", downloaded_path.display());
        }

        if postprocess_result.is_err() {
            let _ = fs::remove_file(&audio_path);
        }

        postprocess_result.map(|_| audio_path)
    })
}

pub fn finalize_download(temp_path: &Path, dest_path: &Path) -> Result<(), Error> {
    if fs::rename(temp_path, dest_path).is_err() {
        fs::copy(temp_path, dest_path).map_err(Error::Finalize)?;
        let _ = fs::remove_file(temp_path);
    }
    Ok(())
}

pub fn discard_temp_file(path: &Path) {
    let _ = fs::remove_file(path);
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
    #[error("failed to create a temporary file for the download")]
    Temp(#[source] std::io::Error),
    #[error("failed to start the download runtime")]
    Runtime(#[source] std::io::Error),
    #[error("failed to move the downloaded file into place")]
    Finalize(#[source] std::io::Error),
    #[error(transparent)]
    YtDlp(#[from] yt_dlp::error::Error),
}
