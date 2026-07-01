use crate::core::error::{Result, TingError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AudiobookshelfChapter {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub title: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookshelfMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub chapters: Vec<AudiobookshelfChapter>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Vec<String>,
    pub genres: Vec<String>,
    pub published_year: Option<String>,
    pub published_date: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub abridged: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExtendedMetadata {
    pub subtitle: Option<String>,
    pub published_year: Option<String>,
    pub published_date: Option<String>,
    pub publisher: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub language: Option<String>,
    pub explicit: bool,
    pub abridged: bool,
    pub tags: Vec<String>, // Added tags here to preserve them
}

impl AudiobookshelfMetadata {
    pub fn new(
        book: &crate::db::models::Book,
        chapters: Vec<AudiobookshelfChapter>,
        extended: ExtendedMetadata,
        series: Vec<String>,
    ) -> Self {
        let tags_vec: Vec<String> = book
            .tags
            .clone()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Use book.year if available, otherwise fall back to extended.published_year
        let published_year = book.year.map(|y| y.to_string()).or(extended.published_year);

        Self {
            tags: tags_vec,
            chapters,
            title: book.title.clone(),
            subtitle: extended.subtitle,
            authors: book.author.clone().map(|s| vec![s]).unwrap_or_default(),
            narrators: book.narrator.clone().map(|s| vec![s]).unwrap_or_default(),
            series,
            genres: book
                .genre
                .clone()
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_default(),
            published_year,
            published_date: extended.published_date,
            publisher: extended.publisher,
            description: book.description.clone(),
            isbn: extended.isbn,
            asin: extended.asin,
            language: extended.language,
            explicit: extended.explicit,
            abridged: extended.abridged,
        }
    }
}

pub fn write_metadata_json(dir: &Path, metadata: &AudiobookshelfMetadata) -> Result<()> {
    let path = dir.join("metadata.json");
    let file = std::fs::File::create(&path).map_err(|e| TingError::IoError(e))?;
    serde_json::to_writer_pretty(file, metadata)
        .map_err(|e| TingError::SerializationError(e.to_string()))?;
    tracing::info!(
        target: "audit::metadata",
        message_key = "metadata.json.write_succeeded",
        message_params = %serde_json::json!({ "path": dir.display().to_string() }),
        path = %dir.display(),
        "Metadata JSON written"
    );
    Ok(())
}

pub fn read_metadata_json(dir: &Path) -> Result<Option<AudiobookshelfMetadata>> {
    let path = dir.join("metadata.json");
    if !path.exists() {
        return Ok(None);
    }
    let file = std::fs::File::open(&path).map_err(|e| TingError::IoError(e))?;
    let metadata: AudiobookshelfMetadata = serde_json::from_reader(file)
        .map_err(|e| TingError::DeserializationError(e.to_string()))?;
    Ok(Some(metadata))
}
