use axum::{Json, response::IntoResponse};
use crate::api::models::tools::{GenerateRegexRequest, GenerateRegexResponse};
use crate::core::error::Result;

pub async fn generate_regex(
    Json(req): Json<GenerateRegexRequest>
) -> Result<impl IntoResponse> {
    let filename = req.filename.trim().to_string();
    let num_str = req.chapter_number.trim().to_string();
    let title_str = req.chapter_title.trim().to_string();
    
    // 1. Find number range
    let num_val = num_str.parse::<i32>().unwrap_or(-1);
    
    // Find all digit sequences in filename
    let re_digits = regex::Regex::new(r"\d+").unwrap();
    let mut num_range = None;
    
    // Try to find the number as digits
    if num_val >= 0 {
        for mat in re_digits.find_iter(&filename) {
            if let Ok(val) = mat.as_str().parse::<i32>() {
                if val == num_val {
                    num_range = Some(mat.range());
                    break; // Take first match
                }
            }
        }
    }
    
    // Fallback: if not found as digits (or num is not integer), search as string literal
    if num_range.is_none() {
        if let Some(idx) = filename.find(&num_str) {
            num_range = Some(idx..idx+num_str.len());
        }
    }
    
    // 2. Find title range
    // Search for title AFTER the number if possible
    let search_start = num_range.as_ref().map(|r| r.end).unwrap_or(0);
    let mut title_range = if let Some(idx) = filename[search_start..].find(&title_str) {
        Some((search_start + idx)..(search_start + idx + title_str.len()))
    } else {
        None
    };
    
    if title_range.is_none() {
         // Search from beginning if not found after
         if let Some(idx) = filename.find(&title_str) {
             title_range = Some(idx..idx+title_str.len());
         }
    }
    
    // 3. Construct regex
    let mut pattern = String::from("^");
    let mut current_idx = 0;
    
    // Sort ranges by start index to process sequentially
    let mut ranges: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
    if let Some(ref r) = num_range { ranges.push((r.clone(), r"(\d+)")); }
    if let Some(ref r) = title_range { ranges.push((r.clone(), r"(.+)")); }
    
    // Sort ranges
    ranges.sort_by_key(|(r, _)| r.start);
    
    // Check for overlap - if overlap, priority to Title or Number?
    // If Title contains Number, we have an issue.
    // Assuming no overlap for now.
    
    for (range, replacement) in ranges {
        if range.start < current_idx {
            // Overlap detected, skip
            continue;
        }
        
        // Escape text before this range
        if range.start > current_idx {
            pattern.push_str(&regex::escape(&filename[current_idx..range.start]));
        }
        pattern.push_str(replacement);
        current_idx = range.end;
    }
    
    // Escape remaining text
    if current_idx < filename.len() {
        pattern.push_str(&regex::escape(&filename[current_idx..]));
    }
    
    // Allow trailing characters if title capture (.+) is at the end?
    // Usually (.+) matches until end if nothing follows.
    // But if filename has extension, we might want to ignore it?
    // User input filename usually excludes extension (as per request "不带后缀").
    // So we anchor to end with $.
    pattern.push('$');
    
    // Test it
    let re = regex::Regex::new(&pattern).unwrap();
    let caps = re.captures(&filename);
    
    let (captured_index, captured_title) = if let Some(ref c) = caps {
        // We need to map capture groups back to index/title
        // If we have 2 groups, 1 is index, 2 is title (based on order of insertion)
        // Wait, insertion order in regex depends on order in string.
        // If Title comes before Number in string, group 1 is Title.
        
        // We need to know which group corresponds to what.
        // Let's check ranges order.
        let mut index_val = None;
        let mut title_val = None;
        
        let mut group_idx = 1;
        // Re-sort ranges to match regex group order
        // We re-create the ranges list same way
        let mut sorted_ranges: Vec<(usize, &str)> = Vec::new();
        if let Some(ref r) = num_range { sorted_ranges.push((r.start, "index")); }
        if let Some(ref r) = title_range { sorted_ranges.push((r.start, "title")); }
        sorted_ranges.sort_by_key(|(start, _)| *start);
        
        for (_, type_name) in sorted_ranges {
            if let Some(m) = c.get(group_idx) {
                let val = m.as_str().to_string();
                if type_name == "index" {
                    index_val = Some(val);
                } else {
                    title_val = Some(val);
                }
            }
            group_idx += 1;
        }
        
        (index_val, title_val)
    } else {
        (None, None)
    };
    
    Ok(Json(GenerateRegexResponse {
        regex: pattern,
        test_match: caps.is_some(),
        captured_index,
        captured_title,
    }))
}
