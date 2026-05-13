// Markdown Chunker — Split Markdown content into semantic chunks

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub text: String,
    pub content_hash: String,
    pub source_file: String,
    pub heading_context: String,
    pub chunk_type: ChunkType,
    pub position: usize,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkType {
    Heading,
    Paragraph,
    List,
    CodeBlock,
}

/// Split Markdown content into semantic chunks at heading boundaries
pub fn chunk_markdown(content: &str, source_file: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_heading = String::new();
    let mut position: usize = 0;

    // Split by double newline (paragraph boundaries) then by headings
    let sections = content.split("\n\n");

    for section in sections {
        let trimmed = section.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Detect heading level within this section
        let (heading, body) = if let Some(_) = trimmed.strip_prefix("# ") {
            (trimmed, "")
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            current_heading = rest.to_string();
            (trimmed, "")
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            current_heading = rest.to_string();
            (trimmed, "")
        } else {
            ("", trimmed)
        };

        let text = if heading.is_empty() { body.to_string() } else { heading.to_string() };
        if text.is_empty() {
            continue;
        }

        let links = extract_links(&text);
        let hash = format!("{:x}", md5::compute(text.as_bytes()));

        chunks.push(Chunk {
            id: format!("chunk_{}_{}", source_file.replace(['/', '\\', '.'], "_"), position),
            text,
            content_hash: hash,
            source_file: source_file.to_string(),
            heading_context: current_heading.clone(),
            chunk_type: if heading.is_empty() { ChunkType::Paragraph } else { ChunkType::Heading },
            position,
            links,
        });
        position += 1;
    }

    chunks
}

fn extract_links(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\[\[([^\]|#]+)").unwrap();
    re.captures_iter(text)
        .filter_map(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_headings() {
        let content = "# Title\n\nSome text.\n\n## Section\n\nMore text.";
        let chunks = chunk_markdown(content, "test.md");
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_chunk_preserves_links() {
        let content = "See [[Rust]] and [[Tauri]] for details.";
        let chunks = chunk_markdown(content, "test.md");
        assert_eq!(chunks[0].links, vec!["Rust", "Tauri"]);
    }
}
