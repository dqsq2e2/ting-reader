use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GenerateRegexRequest {
    pub filename: String,
    pub chapter_number: String,
    pub chapter_title: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateRegexResponse {
    pub regex: String,
    pub test_match: bool,
    pub captured_index: Option<String>,
    pub captured_title: Option<String>,
}
