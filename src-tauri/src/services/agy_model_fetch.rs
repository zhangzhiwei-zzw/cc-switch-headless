//! Discover Antigravity models using the server's local agy installation.

use crate::services::model_fetch::FetchedModel;
use std::collections::BTreeMap;

/// Without a key this asks the signed-in agy. With a key it uses a throwaway
/// `--gemini_dir` so API-key settings never touch the user's official login.
pub async fn get_agy_models(
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<Vec<FetchedModel>, String> {
    let api_key = api_key.filter(|key| !key.trim().is_empty());
    let base_url = base_url.filter(|url| !url.trim().is_empty());
    if api_key.is_none() && crate::agy_config::settings_path().exists() {
        crate::agy_config::read_live_settings().map_err(|error| error.to_string())?;
    }
    let path = tokio::task::spawn_blocking(crate::agy_config::real_agy_bin)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Antigravity CLI not found; install agy first".to_string())?;
    let mut command = tokio::process::Command::new(path);
    command
        .kill_on_drop(true)
        .env_remove("GEMINI_API_KEY")
        .env_remove("GOOGLE_GEMINI_BASE_URL");
    let _scratch = match &api_key {
        Some(key) => {
            let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
            let config = dir.path().join("antigravity-cli");
            std::fs::create_dir_all(&config).map_err(|error| error.to_string())?;
            std::fs::write(
                config.join("settings.json"),
                br#"{"modelProvider":"gemini"}"#,
            )
            .map_err(|error| error.to_string())?;
            command
                .arg(format!("--gemini_dir={}", dir.path().display()))
                .current_dir(dir.path())
                .env("GEMINI_API_KEY", key);
            if let Some(url) = &base_url {
                command.env("GOOGLE_GEMINI_BASE_URL", url);
            }
            Some(dir)
        }
        None => None,
    };
    command.arg("models");
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), command.output())
        .await
        .map_err(|_| "Antigravity model discovery timed out".to_string())?
        .map_err(|_| "Antigravity CLI not found; install agy first".to_string())?;
    if !output.status.success() {
        return Err("Antigravity could not list models. Check the API key and endpoint, or sign in with agy for official models.".into());
    }
    Ok(parse_agy_models(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_agy_models(output: &str) -> Vec<FetchedModel> {
    let mut names = BTreeMap::new();
    for line in output.lines() {
        let Some((id, name)) = line.trim().split_once('\t') else {
            continue;
        };
        let name = name.trim();
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
            || name.is_empty()
            || name.chars().any(char::is_control)
        {
            continue;
        }
        names
            .entry(id.to_string())
            .or_insert_with(|| name.to_string());
    }
    names
        .into_values()
        .map(|id| FetchedModel { id, owned_by: None })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn returns_native_setting_names_and_deduplicates_aliases() {
        let models = super::parse_agy_models(
            "Fetching available models...\ngemini-test\tGemini Test\nclaude-test\tClaude Test\ngemini-test\tDuplicate\n",
        );
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["Claude Test", "Gemini Test"]
        );
    }

    #[test]
    fn skips_malformed_rows_and_control_characters() {
        let models = super::parse_agy_models(
            "notice\n\tNo alias\nbad alias\tName\nempty\t\nspaces\t   \ncontrol\tBad\u{1b}Name\nvalid\t Gemini Test \n",
        );
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "Gemini Test");
    }
}
