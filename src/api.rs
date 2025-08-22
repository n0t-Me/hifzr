use crate::models::{Verse, ChapterResponse};
use anyhow::{Result, Context};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use tokio::time::{sleep, Duration};

const BASE: &str = "https://api.quran.com/api/v4";

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChapterQuery {
    pub audio: u32,
    pub page: Option<u32>,
    pub per_page: Option<u32>,

    // NEW: keep responses skinny & predictable
    pub words: Option<bool>,      // set false
    pub fields: Option<String>,   // ask only what you need
}

async fn get_page(client: &Client, url: &str, pq: &ChapterQuery) -> Result<ChapterResponse> {

    let mut tries = 0u32;
    loop {
        let resp = client.get(url).query(pq).send().await
            .with_context(|| format!("send failed: {url}"))?;

        let status = resp.status();                // <— capture BEFORE any move

        if status == StatusCode::TOO_MANY_REQUESTS && tries < 5 {
            sleep(Duration::from_millis(250 * (1 << tries))).await;
            tries += 1;
            continue;
        }

        // error_for_status() CONSUMES resp, but we no longer need resp itself
        let resp = resp.error_for_status()
            .context(format!("HTTP {status} for {url}"))?;

        return resp.json::<ChapterResponse>().await
            .context("decode ChapterResponse failed");
    }
}


pub async fn fetch_chapter(
    client: &Client,
    audio: u32,
    chapter: u32,
) -> Result<Vec<Verse>> {
    let mut out = Vec::new();
    let mut page = 1u32;
    loop {
        let url = format!("{BASE}/verses/by_chapter/{chapter}");
        let pq = ChapterQuery {
            audio,
            page: Some(page),
            per_page: Some(50),
            words: Some(false),
            fields: Some("juz_number,hizb_number,verse_key,verse_number,rub_el_hizb_number".into()),
        };
        let parsed = get_page(client, &url, &pq).await?;
        if parsed.verses.is_empty() { break; }
        out.extend(parsed.verses);
        match parsed.pagination.and_then(|p| p.next_page) {
            Some(next) => page = next,
            None => break,
        }
    }
    Ok(out)
}
