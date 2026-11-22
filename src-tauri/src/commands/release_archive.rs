// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Public GitHub repo whose Releases feed the in-app release archive.
const REPO: &str = "imperium42/echophrase-releases";

/// Cache TTL: 10 minutes.
const CACHE_TTL: Duration = Duration::from_secs(600);

/// A prior release entry returned to the frontend archive section.
#[derive(Debug, Clone, Serialize)]
pub struct ArchiveRelease {
    pub version: String,
    /// ISO date string: YYYY-MM-DD
    pub date: String,
    pub body: String,
}

type CacheEntry = Option<(Instant, Vec<ArchiveRelease>)>;
static CACHE: LazyLock<Mutex<CacheEntry>> = LazyLock::new(|| Mutex::new(None));

/// Fetch up to 5 prior releases (skipping the latest) from the GitHub Releases API.
///
/// Results are cached in-process for 10 minutes. Any network error, non-200 response,
/// or JSON parse failure returns `Ok(vec![])` (silent-fail contract matching the
/// previous TypeScript implementation).
#[tauri::command]
pub async fn fetch_release_archive() -> Result<Vec<ArchiveRelease>, String> {
    // Check in-memory cache first.
    {
        let guard = CACHE.lock().map_err(|e| e.to_string())?;
        if let Some((cached_at, ref releases)) = *guard {
            if cached_at.elapsed() < CACHE_TTL {
                return Ok(releases.clone());
            }
        }
    }

    let url = format!("https://api.github.com/repos/{}/releases?per_page=6", REPO);
    let version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("Echophrase/{}", version);

    let result: Result<Vec<ArchiveRelease>, String> = async {
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", &user_agent)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("GitHub API returned status: {}", response.status()));
        }

        let releases: Vec<serde_json::Value> = response.json().await.map_err(|e| e.to_string())?;

        // Filter out drafts; keep all non-draft releases (beta and stable).
        let filtered: Vec<&serde_json::Value> = releases
            .iter()
            .filter(|r| !r["draft"].as_bool().unwrap_or(false))
            .collect();

        // Skip the first entry (latest - shown elsewhere) and take the next 5.
        let archive: Vec<ArchiveRelease> = filtered
            .into_iter()
            .skip(1)
            .take(5)
            .map(|r| {
                let version = r["tag_name"].as_str().unwrap_or("").to_string();
                let date = r["published_at"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(10)
                    .collect();
                let body = r["body"].as_str().unwrap_or("").to_string();
                ArchiveRelease {
                    version,
                    date,
                    body,
                }
            })
            .collect();

        Ok(archive)
    }
    .await;

    match result {
        Ok(archive) => {
            let mut guard = CACHE.lock().map_err(|e| e.to_string())?;
            *guard = Some((Instant::now(), archive.clone()));
            Ok(archive)
        }
        Err(e) => {
            log::warn!(
                "fetch_release_archive: failed to fetch GitHub releases: {}",
                e
            );
            Ok(vec![])
        }
    }
}
