use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use assets::asset::application::{AssetApplicationError, AssetInspectedMedia};
use assets::asset::domain::{AssetMediaFacts, AssetMediaKind, AssetMediaMimeType};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

#[derive(Deserialize)]
struct ProbeOutput {
    format: ProbeFormat,
    streams: Vec<ProbeStream>,
    #[serde(default)]
    programs: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct ProbeFormat {
    format_name: String,
    duration: String,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_type: String,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<String>,
    channels: Option<u8>,
}

pub(super) async fn inspect(
    executable: &Path,
    bytes: Vec<u8>,
    expected: AssetMediaKind,
    deadline: Instant,
) -> Result<AssetInspectedMedia, AssetApplicationError> {
    let mut child = Command::new(executable)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name,duration:stream=codec_type,width,height,sample_rate,channels:program",
            "-of",
            "json",
            "-i",
            "pipe:0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| AssetApplicationError::InspectionFailed)?;
    let mut stdin = child.stdin.take().ok_or(AssetApplicationError::InspectionFailed)?;
    let stdout = child.stdout.take().ok_or(AssetApplicationError::InspectionFailed)?;
    let writer = tokio::spawn(async move { stdin.write_all(&bytes).await });
    let reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let status =
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), child.wait()).await;
    let status = match status {
        Ok(result) => result.map_err(|_| AssetApplicationError::InspectionFailed)?,
        Err(_) => {
            writer.abort();
            reader.abort();
            return Err(AssetApplicationError::DeadlineExceeded);
        }
    };
    writer
        .await
        .map_err(|_| AssetApplicationError::InspectionFailed)?
        .map_err(|_| AssetApplicationError::InspectionFailed)?;
    let stdout = reader
        .await
        .map_err(|_| AssetApplicationError::InspectionFailed)?
        .map_err(|_| AssetApplicationError::InspectionFailed)?;
    if !status.success() || stdout.len() > 1024 * 1024 {
        return Err(AssetApplicationError::InspectionFailed);
    }
    let probe: ProbeOutput =
        serde_json::from_slice(&stdout).map_err(|_| AssetApplicationError::InvalidMedia)?;
    interpret(probe, expected)
}

fn interpret(
    probe: ProbeOutput,
    expected: AssetMediaKind,
) -> Result<AssetInspectedMedia, AssetApplicationError> {
    if probe.programs.len() > 1 {
        return Err(AssetApplicationError::InvalidMedia);
    }
    let duration_ms = duration_ms(&probe.format.duration)?;
    match expected {
        AssetMediaKind::Image => Err(AssetApplicationError::InvalidMedia),
        AssetMediaKind::Video => video(probe, duration_ms),
        AssetMediaKind::Audio => audio(probe, duration_ms),
    }
}

fn video(
    probe: ProbeOutput,
    duration_ms: u64,
) -> Result<AssetInspectedMedia, AssetApplicationError> {
    let mime = match probe.format.format_name.as_str() {
        name if name
            .split(',')
            .any(|part| matches!(part, "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2")) =>
        {
            AssetMediaMimeType::VideoMp4
        }
        name if name.split(',').any(|part| part == "matroska" || part == "webm") => {
            AssetMediaMimeType::VideoWebm
        }
        _ => return Err(AssetApplicationError::InvalidMedia),
    };
    let videos =
        probe.streams.iter().filter(|stream| stream.codec_type == "video").collect::<Vec<_>>();
    if videos.len() != 1 {
        return Err(AssetApplicationError::InvalidMedia);
    }
    let video = videos[0];
    let facts = AssetMediaFacts::try_video(
        video.width.ok_or(AssetApplicationError::InvalidMedia)?,
        video.height.ok_or(AssetApplicationError::InvalidMedia)?,
        duration_ms,
        probe.streams.iter().any(|stream| stream.codec_type == "audio"),
    )
    .map_err(|_| AssetApplicationError::InvalidMedia)?;
    AssetInspectedMedia::try_new(mime, facts)
}

fn audio(
    probe: ProbeOutput,
    duration_ms: u64,
) -> Result<AssetInspectedMedia, AssetApplicationError> {
    let mime = match probe.format.format_name.as_str() {
        "mp3" => AssetMediaMimeType::AudioMpeg,
        "wav" => AssetMediaMimeType::AudioWav,
        "ogg" => AssetMediaMimeType::AudioOgg,
        _ => return Err(AssetApplicationError::InvalidMedia),
    };
    let streams =
        probe.streams.iter().filter(|stream| stream.codec_type == "audio").collect::<Vec<_>>();
    if streams.len() != 1 || probe.streams.iter().any(|stream| stream.codec_type == "video") {
        return Err(AssetApplicationError::InvalidMedia);
    }
    let stream = streams[0];
    let sample_rate = stream
        .sample_rate
        .as_deref()
        .ok_or(AssetApplicationError::InvalidMedia)?
        .parse::<u32>()
        .map_err(|_| AssetApplicationError::InvalidMedia)?;
    let facts = AssetMediaFacts::try_audio(
        duration_ms,
        sample_rate,
        stream.channels.ok_or(AssetApplicationError::InvalidMedia)?,
    )
    .map_err(|_| AssetApplicationError::InvalidMedia)?;
    AssetInspectedMedia::try_new(mime, facts)
}

fn duration_ms(value: &str) -> Result<u64, AssetApplicationError> {
    let seconds = value.parse::<f64>().map_err(|_| AssetApplicationError::InvalidMedia)?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(AssetApplicationError::InvalidMedia);
    }
    let milliseconds = (seconds * 1000.0).round();
    if milliseconds > u64::MAX as f64 {
        return Err(AssetApplicationError::InvalidMedia);
    }
    Ok(milliseconds as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(
        json: &str,
        kind: AssetMediaKind,
    ) -> Result<AssetInspectedMedia, AssetApplicationError> {
        interpret(serde_json::from_str(json).unwrap(), kind)
    }

    #[test]
    fn video_and_audio_probe_facts_are_strictly_interpreted() {
        let video = parse(
            r#"{"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"1.250"},"streams":[{"codec_type":"video","width":640,"height":480},{"codec_type":"audio"}],"programs":[]}"#,
            AssetMediaKind::Video,
        )
        .unwrap();
        assert_eq!(video.mime_type(), AssetMediaMimeType::VideoMp4);
        let AssetMediaFacts::Video(facts) = video.media_facts() else { panic!("video facts") };
        assert_eq!(facts.duration_ms(), 1_250);
        assert!(facts.has_audio());

        let audio = parse(
            r#"{"format":{"format_name":"ogg","duration":"2"},"streams":[{"codec_type":"audio","sample_rate":"48000","channels":2}],"programs":[]}"#,
            AssetMediaKind::Audio,
        )
        .unwrap();
        assert_eq!(audio.mime_type(), AssetMediaMimeType::AudioOgg);
    }

    #[test]
    fn unknown_duration_multiple_video_streams_and_container_mismatch_fail_closed() {
        for json in [
            r#"{"format":{"format_name":"mp4","duration":"N/A"},"streams":[],"programs":[]}"#,
            r#"{"format":{"format_name":"mp4","duration":"1"},"streams":[{"codec_type":"video","width":1,"height":1},{"codec_type":"video","width":1,"height":1}],"programs":[]}"#,
            r#"{"format":{"format_name":"mp3","duration":"1"},"streams":[{"codec_type":"video","width":1,"height":1}],"programs":[]}"#,
        ] {
            assert_eq!(
                parse(json, AssetMediaKind::Video),
                Err(AssetApplicationError::InvalidMedia)
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bundled_process_contract_uses_stdin_and_parses_bounded_json() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("ffprobe-fixture");
        std::fs::write(
            &executable,
            "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"format\":{\"format_name\":\"wav\",\"duration\":\"1\"},\"streams\":[{\"codec_type\":\"audio\",\"sample_rate\":\"48000\",\"channels\":2}],\"programs\":[]}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();

        let inspected = inspect(
            &executable,
            b"private media bytes".to_vec(),
            AssetMediaKind::Audio,
            Instant::now() + std::time::Duration::from_secs(2),
        )
        .await
        .unwrap();
        assert_eq!(inspected.mime_type(), AssetMediaMimeType::AudioWav);
    }
}
