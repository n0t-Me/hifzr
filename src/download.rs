use anyhow::{Context};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};


fn base_dir(root: &str) -> PathBuf {
    Path::new(root).to_path_buf()
}

fn resolve_audio_url(u: &str) -> String {
    if u.starts_with("http") { u.to_string() } else { format!("https://audio.qurancdn.com/{}", u.trim_start_matches('/')) }
}

pub async fn run_filter(
    client: &reqwest::Client,
    reciter: u32,
    chapter: u32,
    out_root: &str,
    force: bool,
    only_verses: Option<&[u32]>,
) -> anyhow::Result<()> {

    //let dir = chapter_dir(out_root, chapter);
    let dir = base_dir(out_root);
    fs::create_dir_all(&dir).await?;

    let verses = crate::api::fetch_chapter(client, reciter, chapter).await
        .with_context(|| format!("fetch_chapter failed for surah {}", chapter))?;

    let wanted: Option<std::collections::HashSet<u32>> = only_verses.map(|v| v.iter().copied().collect());

    // tiny progress
    let total = verses.len();
    let mut done = 0usize;

    for v in verses {
        if let Some(w) = &wanted {
            if !w.contains(&v.verse_number) { continue; }
        }

        let ayah = v.verse_number;
        let mp3 = dir.join(format!("{:03}.mp3", ayah));
        let seg = dir.join(format!("{:03}.segments.json", ayah));

        if force || !mp3.exists() {
            let url = resolve_audio_url(&v.audio.url);
            let resp = client.get(&url).send().await?
                .error_for_status()
                .with_context(|| format!("GET {}", url))?;
            let mut f = fs::File::create(&mp3).await?;
            let mut s = resp.bytes_stream();
            while let Some(chunk) = s.next().await { f.write_all(&chunk?).await?; }
        }

        let pairs: Vec<[u32; 2]> = match v.audio.segments.as_ref() {
            // If your model is: Option<Vec<Segment>>
            Some(segs) => segs
                .iter()
                .filter_map(|s| {
                    let (sms, ems) = (s.start_ms, s.end_ms);
                    (ems > sms).then_some([sms, ems])
                })
                .collect(),
            None => Vec::new(),
        };
        let data = serde_json::to_vec(&pairs)?;
        tokio::fs::write(&seg, data).await?;

        done += 1;
        eprint!("\rprepping {:03}: {}/{}", ayah, done, total);
    }
    eprintln!();
    Ok(())
}
