use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChapterResponse {
    pub verses: Vec<Verse>,
    pub pagination: Option<Pagination>, // make it optional to avoid surprises
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pagination {
    pub per_page: Option<u32>,
    pub current_page: Option<u32>,
    pub next_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub total_records: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Verse {
    pub id: u32,
    pub verse_number: u32,
    pub verse_key: String,

    // Make these optional unless you guarantee them via ?fields=
    pub hizb_number: Option<u32>,
    #[serde(alias = "rub_number", alias = "rub_el_hizb_number")]
    pub rub_el_hizb_number: Option<u32>,
    pub ruku_number: Option<u32>,
    pub manzil_number: Option<u32>,
    pub sajdah_number: Option<u32>,

    // page_number is deprecated in the API; can be missing
    pub page_number: Option<u32>,
    pub juz_number: Option<u32>,

    pub audio: Audio,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Audio {
    pub url: String,

    // Accept either [start,end] or [i,j,start,end]
    #[serde(default)]
    pub segments: Option<Vec<Segment>>,
}

#[derive(Clone, Serialize, Debug)]
pub struct Segment {
    pub i: Option<u32>,
    pub j: Option<u32>,
    pub start_ms: u32,
    pub end_ms: u32,
}

impl<'de> Deserialize<'de> for Segment {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw { Two([u32; 2]), Four([u32; 4]) }
        match Raw::deserialize(de)? {
            Raw::Two([s, e]) => Ok(Segment { i: None, j: None, start_ms: s, end_ms: e }),
            Raw::Four([i, j, s, e]) => Ok(Segment { i: Some(i), j: Some(j), start_ms: s, end_ms: e }),
        }
    }
}
