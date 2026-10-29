use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;

use super::{record::default_comment, TitleInfo};

pub(super) async fn load(directory: &Path) -> Result<TitleInfo> {
    let content = match tokio::fs::read_to_string(directory.join("info.json")).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TitleInfo::default());
        }
        Err(error) => return Err(error.into()),
    };
    let mut info: TitleInfo = match serde_json::from_str(&content) {
        Ok(info) => info,
        Err(error) => {
            set_aside(directory, &error).await?;
            TitleInfo::default()
        }
    };
    if info.comment.is_empty() {
        info.comment = default_comment();
    }
    Ok(info)
}

/// Move a malformed `info.json` aside so the next save cannot overwrite the
/// only copy of its bytes.
async fn set_aside(directory: &Path, error: &serde_json::Error) -> Result<()> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let backup = directory.join(format!("info.json.malformed-{nanos}"));
    match tokio::fs::rename(directory.join("info.json"), &backup).await {
        Ok(()) => {
            tracing::warn!(
                "Set aside malformed {} as {}: {}",
                directory.join("info.json").display(),
                backup.display(),
                error
            );
            Ok(())
        }
        // Another metadata store already moved it.
        Err(rename_error) if rename_error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(rename_error) => Err(rename_error.into()),
    }
}

pub(super) async fn save(directory: &Path, info: &TitleInfo) -> Result<()> {
    let json = serde_json::to_string_pretty(info)?;
    let temp = directory.join(format!(".info.json.{:08x}.tmp", rand::random::<u32>()));
    let written = async {
        tokio::fs::write(&temp, json).await?;
        tokio::fs::rename(&temp, directory.join("info.json")).await
    }
    .await;
    if let Err(error) = written {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_aside_treats_an_already_moved_file_as_done() {
        let directory = tempfile::tempdir().unwrap();
        let error = serde_json::from_str::<TitleInfo>("not json").unwrap_err();
        set_aside(directory.path(), &error).await.unwrap();
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
