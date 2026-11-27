use crate::models::{UpdateAsset, UpdateInfo};
use std::time::Duration;

fn parse_version(raw: &str) -> Vec<u64> {
    let cleaned = raw.trim().trim_start_matches('v');
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    core.split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer(latest: &str, current: &str) -> bool {
    let a = parse_version(latest);
    let b = parse_version(current);
    for index in 0..a.len().max(b.len()) {
        let left = *a.get(index).unwrap_or(&0);
        let right = *b.get(index).unwrap_or(&0);
        if left != right {
            return left > right;
        }
    }
    false
}

pub async fn check(repo: &str, current: &str) -> Result<UpdateInfo, String> {
    let repo = repo.trim().trim_matches('/');
    if !repo.contains('/') {
        return Err("שם המאגר בהגדרות צריך להיות בפורמט owner/repo".into());
    }

    let client = reqwest::Client::builder()
        .user_agent(format!("WorkTime/{current}"))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("לא נמצאה מהדורה עבור {repo}. ודאו שהמאגר קיים ושהועלתה אליו מהדורה."));
    }
    if !response.status().is_success() {
        return Err(format!("שרת GitHub החזיר סטטוס {}", response.status()));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("תשובת GitHub אינה תקינה: {e}"))?;

    let tag = payload
        .get("tag_name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let latest = tag.trim_start_matches('v').to_string();
    let notes = payload
        .get("body")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let title = payload
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("גרסה {latest}"));
    let page_url = payload
        .get("html_url")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let published_at = payload
        .get("published_at")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    let mut assets = Vec::new();
    if let Some(list) = payload.get("assets").and_then(|value| value.as_array()) {
        for asset in list {
            let name = asset
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let is_installer =
                name.ends_with(".exe") || name.ends_with(".msi") || name.ends_with(".msi.zip");
            if !is_installer {
                continue;
            }
            assets.push(UpdateAsset {
                name,
                url: asset
                    .get("browser_download_url")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                size: asset
                    .get("size")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
            });
        }
    }

    Ok(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.clone(),
        has_update: is_newer(&latest, current),
        title,
        notes,
        page_url,
        published_at,
        assets,
    })
}
