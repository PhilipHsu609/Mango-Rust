use std::path::Path;

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
    let mut info: TitleInfo = serde_json::from_str(&content).unwrap_or_default();
    if info.comment.is_empty() {
        info.comment = default_comment();
    }
    Ok(info)
}

pub(super) async fn save(directory: &Path, info: &TitleInfo) -> Result<()> {
    let json = serde_json::to_string_pretty(info)?;
    tokio::fs::write(directory.join("info.json"), json).await?;
    Ok(())
}
