use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::Client;
use std::path::PathBuf;

// ✨ colors
use owo_colors::OwoColorize;

mod models;
mod api;
mod download;
mod hifz;
mod lookup;

#[derive(Parser)]
#[command(
    name = "hifzr",
    version,
    about = "Hifz-friendly Quran downloader & ayah playlist builder",
    arg_required_else_help = true,
    // clap colors are for help/usage; our runtime colors handled by owo-colors
    color = clap::ColorChoice::Auto
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download ayahs for a given chapter/reciter into a neat folder
    Download {
        #[arg(long)] reciter: String,
        #[arg(long)] chapter: String,
        #[arg(long, default_value="~/Music/Quran_hifz")] out: String,
        #[arg(long, default_value_t=false)] force: bool,
    },
    /// Build an ayah-only playlist (optionally auto-download first)
    Hifz {
        #[arg(long)] chapter: String,
        /// "1-5,7,10-12"; if omitted we scan the folder
        #[arg(long)] verses: Option<String>,

        /// Auto-download missing files first (needs --reciter)
        #[arg(long, default_value_t=false)] auto_download: bool,
        #[arg(long)] reciter: Option<String>,
        #[arg(long, default_value_t=false)] force: bool,

        /// Repeats per ayah
        #[arg(long, default_value_t=3)] repeat: usize,

        /// Optional silence (ms) between repeats/ayahs (uses a tiny silent file)
        #[arg(long, default_value_t=0)] gap_ms: u32,

        #[arg(long, default_value="~/Music/Quran_hifz")] out: String,
    },
    /// List chapters or reciters
    Ls {
        #[arg(value_enum)] what: ListWhat,
    },
}

#[derive(Copy, Clone, Eq, PartialEq, ValueEnum)]
enum ListWhat { Chapters, Reciters }

// ---------- small UI helpers ----------
fn expand_tilde(p: &str) -> String {
    if p.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(&p[2..]).to_string_lossy().to_string();
        }
    }
    p.to_string()
}

fn per_surah_base(out: &str, surah_slug: &str, reciter_slug: &str) -> String {
    let mut base = PathBuf::from(expand_tilde(out));
    base.push(surah_slug);
    base.push(reciter_slug);
    base.to_string_lossy().to_string()
}

fn label(s: &str) -> String { s.dimmed().to_string() }

// ---------- main ----------
#[tokio::main]
async fn main() -> Result<()> {
    // Respect NO_COLOR if the user wants plain output (owo-colors honors OWO_COLORS=0)
    if std::env::var_os("NO_COLOR").is_some() {
        unsafe {
            std::env::set_var("OWO_COLORS", "0");
        }
    }

    let cli = Cli::parse();
    let client = Client::new();

    match cli.cmd {
        Cmd::Download { reciter, chapter, out, force } => {
            let chapters = lookup::fetch_chapters(&client).await?;
            let c = lookup::resolve_chapter(&chapters, &chapter)
                .with_context(|| format!("{} {}", "Unknown chapter:".red().bold(), chapter.bold()))?;
            let surah_slug = lookup::chapter_slug(c);
            let surah_display = &c.name_complex;

            let reciters = lookup::fetch_reciters(&client).await?;
            let r = lookup::resolve_reciter(&reciters, &reciter)
                .with_context(|| format!("{} {}", "Unknown reciter:".red().bold(), reciter.bold()))?;
            let rslug = lookup::slugify(&r.reciter_name);

            let out_root = per_surah_base(&out, &surah_slug, &rslug);

            println!(
                "{} {} {} {}",
                "".bright_black(),
                label("Downloading →"),
                format!("{}", surah_display).bold().cyan(),
                format!("({:03} · {})", c.id, c.name_simple).dimmed()
            );
            println!(
                "   {} {}",
                label("Reciter:"),
                format!("{}", r.reciter_name).bold().magenta()
            );
            println!(
                "   {} {}",
                label("Folder:"),
                out_root.to_string().bold().blue()
            );

            download::run_filter(&client, r.id, c.id, &out_root, force, None).await?;

            println!(
                "{} {} {}",
                "✔".green().bold(),
                "Saved under".bold(),
                out_root.bold().blue()
            );
        }

        Cmd::Hifz { chapter, verses, auto_download, reciter, force, repeat, gap_ms, out } => {
            let chapters = lookup::fetch_chapters(&client).await?;
            let c = lookup::resolve_chapter(&chapters, &chapter)
                .with_context(|| format!("{} {}", "Unknown chapter:".red().bold(), chapter.bold()))?;
            let surah_slug = lookup::chapter_slug(c);
            let surah_display = &c.name_complex;

            // where we write/read files
            let out_base = if auto_download || reciter.is_some() {
                let rec = reciter.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("{}",
                        "--reciter is required with --auto-download".yellow().bold()))?;
                let reciters = lookup::fetch_reciters(&client).await?;
                let r = lookup::resolve_reciter(&reciters, rec)
                    .with_context(|| format!("{} {}", "Unknown reciter:".red().bold(), rec.bold()))?;
                let rslug = lookup::slugify(&r.reciter_name);
                println!(
                    "{} {} {}",
                    "".bright_black(),
                    label("Auto-download for"),
                    r.reciter_name.bold().magenta()
                );
                let rec_base = per_surah_base(&out, &surah_slug, &rslug);

                // optional filter list for download
                let only = verses.as_deref()
                    .map(|spec| hifz::parse_verses_spec(spec))
                    .transpose()?.map(|v| v.into_boxed_slice());
                let only_ref = only.as_deref().map(|b| &b[..]);

                download::run_filter(&client, r.id, c.id, &rec_base, force, only_ref).await?;
                rec_base
            } else {
                std::path::PathBuf::from(expand_tilde(&out))
                    .join(&surah_slug)
                    .to_string_lossy().to_string()
            };

            let m3u = hifz::build_ayah_playlist(&out_base, c.id, verses.as_deref(), repeat, gap_ms)?;
            println!(
                "{} {} {}",
                "📝".yellow(),
                "Playlist".bold(),
                m3u.to_string_lossy().bold().blue()
            );
            println!(
                "   {} {} {}",
                label("Surah:"),
                format!("{}", surah_display).bold().cyan(),
                format!("({:03} · {})", c.id, c.name_simple).dimmed()
            );
            println!(
                "   {} {}  {} {}",
                label("Repeat:"),
                repeat.to_string().bold(),
                label("Gap:"),
                if gap_ms == 0 { "none".bold().to_string() } else { format!("{} ms", gap_ms).bold().to_string() }
            );
        }

Cmd::Ls { what } => {
    match what {
        ListWhat::Chapters => {
            let ch = lookup::fetch_chapters(&client).await?;
            println!("{}", "Chapters".bold().cyan());
            for c in ch {
                let id_text = format!("{:>3}", c.id);
                let simple_text = format!("[{}]", c.name_simple);

                println!(
                    "{}  {}  {}",
                    id_text.magenta().bold(),      // color at the call site
                    c.name_complex.bold(),         // borrow field directly
                    simple_text.dimmed(),          // color a bound String
                );
            }
        }
        ListWhat::Reciters => {
            let rs = lookup::fetch_reciters(&client).await?;
            println!("{}", "Reciters".bold().magenta());
            for r in rs {
                let id_text = format!("{:>3}", r.id);
                if let Some(style) = r.style.as_deref().filter(|s| !s.is_empty()) {
                    let style_text = format!("({})", style);
                    println!(
                        "{}  {}  {}",
                        id_text.magenta().bold(),
                        r.reciter_name.bold(),
                        style_text.dimmed(),
                    );
                } else {
                    println!(
                        "{}  {}",
                        id_text.magenta().bold(),
                        r.reciter_name.bold(),
                    );
                }
            }
        }
    }
}

    }

    Ok(())
}
