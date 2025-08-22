use anyhow::{Result};
use reqwest::Client;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

/// Chapters (surahs)
#[derive(Debug, Clone, Deserialize)]
pub struct Chapter {
    pub id: u32,
    pub name_simple: String,         // canonical simple English name
    #[serde(default)]
    pub name_complex: String,        // nicer display form (from API)
    // #[serde(default)]
    // pub name_arabic: String,
}

#[derive(Debug, Deserialize)]
struct ChaptersResp { }//chapters: Vec<Chapter> }

/// Reciters list (for audio "recitation id")
#[derive(Debug, Clone, Deserialize)]
pub struct Reciter {
    pub id: u32,
    #[serde(default)]
    pub reciter_name: String,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecitersResp { }//recitations: Vec<Reciter> }
// add at top
// keep existing Reciter, RecitersResp, etc.

pub fn norm_key(s: &str) -> String {
    s.nfkd().filter(|c| c.is_ascii()).collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in s.nfkd().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() { out.push(ch); dash = false; }
        else if !dash { out.push('-'); dash = true; }
    }
    out.trim_matches('-').to_string()
}

// canonical API fetches
const BASE: &str = "https://api.quran.com/api/v4";

pub async fn fetch_chapters(client: &Client) -> Result<Vec<Chapter>> {
    #[derive(Deserialize)] struct R { chapters: Vec<Chapter> }
    let url = format!("{BASE}/chapters?language=en");
    Ok(client.get(url).send().await?.error_for_status()?.json::<R>().await?
        .chapters)
}

pub async fn fetch_reciters(client: &Client) -> Result<Vec<Reciter>> {
    #[derive(Deserialize)] struct R { recitations: Vec<Reciter> }
    let url = format!("{BASE}/resources/recitations?language=en");
    Ok(client.get(url).send().await?.error_for_status()?.json::<R>().await?
        .recitations)
}

// resolve using SERVER names (not your input)
pub fn resolve_chapter<'a>(chapters: &'a [Chapter], spec: &str) -> Option<&'a Chapter> {
    if let Ok(n) = spec.parse::<u32>() { return chapters.iter().find(|c| c.id == n); }
    let key = norm_key(spec);
    chapters.iter().find(|c| norm_key(&c.name_simple) == key
        || norm_key(&c.name_complex) == key)
}

pub fn resolve_reciter<'a>(reciters: &'a [Reciter], spec: &str) -> Option<&'a Reciter> {
    if let Ok(n) = spec.parse::<u32>() { return reciters.iter().find(|r| r.id == n); }
    let key = norm_key(spec);
    reciters.iter().find(|r| {
        let n = norm_key(&r.reciter_name);
        let s = r.style.as_deref().map(norm_key).unwrap_or_default();
        n.contains(&key) || key.contains(&n) || (!s.is_empty() && s == key)
    })
}

// build canonical folder slug for a surah from SERVER data
pub fn chapter_slug(c: &Chapter) -> String {
    // include the number to avoid ambiguous duplicates between translations
    format!("{}-{:03}", slugify(&c.name_simple), c.id)
}