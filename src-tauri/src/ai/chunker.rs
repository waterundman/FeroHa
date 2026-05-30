// Markdown Chunker — Split Markdown content into semantic chunks

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    let mut heading_stack: Vec<String> = Vec::new();
    let mut position: usize = 0;

    let body_offset = crate::parser::frontmatter::parse_frontmatter(content)
        .map(|(_, offset)| offset)
        .unwrap_or(0);
    let content = &content[body_offset..];

    // Split by double newline (paragraph boundaries) then by headings
    let sections = content.split("\n\n");

    for section in sections {
        let trimmed = section.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Detect heading level within this section
        let (heading, body, level) = if let Some(rest) = trimmed.strip_prefix("# ") {
            (trimmed, rest, 1)
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            (trimmed, rest, 2)
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            (trimmed, rest, 3)
        } else if let Some(rest) = trimmed.strip_prefix("#### ") {
            (trimmed, rest, 4)
        } else if let Some(rest) = trimmed.strip_prefix("##### ") {
            (trimmed, rest, 5)
        } else if let Some(rest) = trimmed.strip_prefix("###### ") {
            (trimmed, rest, 6)
        } else {
            ("", trimmed, 0)
        };

        // Update heading stack based on level
        if level > 0 {
            // Truncate stack to the appropriate level
            heading_stack.truncate(level - 1);
            heading_stack.push(body.to_string());
        }

        let text = if heading.is_empty() {
            body.to_string()
        } else {
            heading.to_string()
        };
        if text.is_empty() {
            continue;
        }

        let links = extract_links(&text);
        let hash = format!("{:x}", md5::compute(text.as_bytes()));

        chunks.push(Chunk {
            id: format!(
                "chunk_{}_{}",
                source_file.replace(['/', '\\', '.'], "_"),
                position
            ),
            text,
            content_hash: hash,
            source_file: source_file.to_string(),
            heading_context: heading_stack.join(" > "),
            chunk_type: if heading.is_empty() {
                ChunkType::Paragraph
            } else {
                ChunkType::Heading
            },
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

    #[test]
    fn test_deep_heading_levels() {
        let content = "# Title\n\n## Section\n\n### Subsection\n\n#### Sub-subsection\n\n##### Deep\n\n###### Deepest";
        let chunks = chunk_markdown(content, "test.md");

        // Check heading contexts
        let heading_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Heading)
            .collect();

        assert_eq!(heading_chunks[0].heading_context, "Title");
        assert_eq!(heading_chunks[1].heading_context, "Title > Section");
        assert_eq!(
            heading_chunks[2].heading_context,
            "Title > Section > Subsection"
        );
        assert_eq!(
            heading_chunks[3].heading_context,
            "Title > Section > Subsection > Sub-subsection"
        );
        assert_eq!(
            heading_chunks[4].heading_context,
            "Title > Section > Subsection > Sub-subsection > Deep"
        );
        assert_eq!(
            heading_chunks[5].heading_context,
            "Title > Section > Subsection > Sub-subsection > Deep > Deepest"
        );
    }

    #[test]
    fn test_heading_stack_truncation() {
        let content = "# Title\n\n## Section\n\n### Subsection\n\n## Another Section\n\n### Another Subsection";
        let chunks = chunk_markdown(content, "test.md");

        let heading_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Heading)
            .collect();

        assert_eq!(heading_chunks[0].heading_context, "Title");
        assert_eq!(heading_chunks[1].heading_context, "Title > Section");
        assert_eq!(
            heading_chunks[2].heading_context,
            "Title > Section > Subsection"
        );
        assert_eq!(heading_chunks[3].heading_context, "Title > Another Section");
        assert_eq!(
            heading_chunks[4].heading_context,
            "Title > Another Section > Another Subsection"
        );
    }

    #[test]
    fn test_paragraph_heading_context() {
        let content = "# Title\n\nSome text.\n\n## Section\n\nMore text.";
        let chunks = chunk_markdown(content, "test.md");

        // Find paragraph chunks
        let paragraph_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Paragraph)
            .collect();

        // First paragraph should have "Title" context
        assert_eq!(paragraph_chunks[0].heading_context, "Title");
        // Second paragraph should have "Title > Section" context
        assert_eq!(paragraph_chunks[1].heading_context, "Title > Section");
    }
}
