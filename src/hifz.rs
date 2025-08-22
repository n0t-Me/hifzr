use anyhow::{Result};
use std::{fs::File, io::Write, path::{Path, PathBuf}, process::Command};


fn base_dir(root: &str) -> PathBuf {
    Path::new(root).to_path_buf()
}

// Detect available ayahs by scanning *.mp3 in the chapter dir
fn detect_available_ayahs(dir: &Path) -> Result<Vec<u32>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.extension().and_then(|s| s.to_str()) != Some("mp3") { continue; }
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Ok(n) = stem.parse::<u32>() { v.push(n); }
        }
    }
    v.sort_unstable();
    v.dedup();
    Ok(v)
}


// Parse "1-5,7,10-12" → sorted unique list
pub fn parse_verses_spec(spec: &str) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((a,b)) = part.split_once('-') {
            let a: u32 = a.parse()?;
            let b: u32 = b.parse()?;
            if a > b { anyhow::bail!("bad range: {part}"); }
            out.extend(a..=b);
        } else {
            out.push(part.parse()?);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

fn ensure_silence_mp3(out_root: &Path, gap_ms: u32) -> Option<PathBuf> {
    if gap_ms == 0 { return None; }
    let name = format!(".silence_{}ms.mp3", gap_ms);
    let path = out_root.join(name);
    if path.exists() { return Some(path); }

    // try to generate via ffmpeg
    let dur = format!("{:.3}", gap_ms as f32 / 1000.0);
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner","-loglevel","error",
            "-f","lavfi","-i","anullsrc=r=48000:cl=mono",
            "-t",&dur,
            "-c:a","libmp3lame","-q:a","9",
        ])
        .arg(&path)
        .status();

    match status {
        Ok(s) if s.success() => Some(path),
        _ => None,
    }
}

pub fn build_ayah_playlist(
    out_root: &str,
    _chapter: u32,               // kept for filename consistency if you want
    verses: Option<&str>,
    repeat: usize,
    gap_ms: u32,
) -> Result<PathBuf> {
    let dir = base_dir(out_root);
    let list = match verses {
        Some(spec) => parse_verses_spec(spec)?,
        None => detect_available_ayahs(&dir)?,
    };

    let silence = ensure_silence_mp3(&dir, gap_ms);

    let m3u = dir.join("hifz_ayah.m3u"); // simple stable name
    let mut f = File::create(&m3u)?;
    writeln!(f, "#EXTM3U")?;

    for ayah in list {
        let mp3 = dir.join(format!("{:03}.mp3", ayah));
        if !mp3.exists() {
            eprintln!("skip {:03}: missing {}", ayah, mp3.display());
            continue;
        }
        for r in 0..repeat {
            writeln!(f, "{}", mp3.display())?;
            // insert silence between repeats (and between ayahs) except after the last repeat
            if let Some(s) = silence.as_ref() {
                if (gap_ms).abs_diff(0) > 500 {
                    if r + 1 < repeat { writeln!(f, "{}", s.display())?; }
                }
            }
        }
        if let Some(s) = silence.as_ref() {
            // gap between ayahs
            if (gap_ms).abs_diff(0) > 500 {
                writeln!(f, "{}", s.display())?;
            }
        }
    }

    // pointer for quick playback scripts / waybar
    std::fs::write(dir.join("latest_playlist.txt"), m3u.to_string_lossy().as_bytes())?;
    Ok(m3u)
}
