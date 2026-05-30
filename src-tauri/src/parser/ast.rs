// Markdown AST Parser — pulldown-cmark wrapper with structural extraction

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Structured representation of a Markdown document's AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownAst {
    pub headings: Vec<Heading>,
    pub links: Vec<Wikilink>,
    pub blocks: Vec<ContentBlock>,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub block_id: String,
    pub position: usize, // byte offset in source
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wikilink {
    pub target: String,
    pub display: Option<String>,
    pub source_file: String,
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub id: String,
    pub block_type: BlockType,
    pub text: String,
    pub parent_heading: Option<String>,
    pub position: usize,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockType {
    Paragraph,
    Heading,
    List,
    CodeBlock,
    Blockquote,
    Table,
}

/// Parse a Markdown string into structured AST
#[allow(dead_code)]
pub fn parse_markdown(content: &str) -> MarkdownAst {
    let parser = Parser::new(content);
    let mut ast = MarkdownAst {
        headings: Vec::new(),
        links: Vec::new(),
        blocks: Vec::new(),
        raw_text: content.to_string(),
    };

    let mut current_heading: Option<String> = None;
    let mut current_text = String::new();
    let mut block_position = 0usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Flush previous text block
                flush_block(
                    &mut ast,
                    &mut current_text,
                    &current_heading,
                    block_position,
                );
                block_position += current_text.len();
                current_text.clear();

                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                ast.headings.push(Heading {
                    level,
                    text: String::new(), // filled in on End
                    block_id: generate_block_id(""),
                    position: block_position,
                });
            }
            Event::Start(_) => {}
            Event::End(tag) => match tag {
                TagEnd::Heading(level) => {
                    if let Some(heading) = ast.headings.last_mut() {
                        current_heading = Some(heading.text.clone());
                    }

                    let _ = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    ast.blocks.push(ContentBlock {
                        id: generate_block_id(&current_text),
                        block_type: BlockType::Heading,
                        text: current_text.clone(),
                        parent_heading: current_heading.clone(),
                        position: block_position,
                        links: Vec::new(),
                    });
                    block_position += current_text.len();
                    current_text.clear();
                }
                TagEnd::Paragraph => {
                    ast.blocks.push(ContentBlock {
                        id: generate_block_id(&current_text),
                        block_type: BlockType::Paragraph,
                        text: current_text.clone(),
                        parent_heading: current_heading.clone(),
                        position: block_position,
                        links: extract_links_from_text(&current_text),
                    });
                    block_position += current_text.len();
                    current_text.clear();
                }
                _ => {}
            },
            Event::Text(text) => {
                current_text.push_str(&text);
            }
            Event::Code(code) => {
                current_text.push('`');
                current_text.push_str(&code);
                current_text.push('`');
            }
            _ => {}
        }
    }

    // Flush remaining text
    flush_block(
        &mut ast,
        &mut current_text,
        &current_heading,
        block_position,
    );

    // Extract [[wikilinks]] from raw text
    ast.links = extract_wikilinks(content, "");

    ast
}

#[allow(dead_code)]
fn flush_block(
    ast: &mut MarkdownAst,
    text: &mut String,
    heading: &Option<String>,
    position: usize,
) {
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        ast.blocks.push(ContentBlock {
            id: generate_block_id(&trimmed),
            block_type: BlockType::Paragraph,
            text: trimmed.clone(),
            parent_heading: heading.clone(),
            position,
            links: extract_links_from_text(&trimmed),
        });
    }
    text.clear();
}

/// Generate a content-based block ID (first 8 hex chars of SHA256)
pub fn generate_block_id(content: &str) -> String {
    if content.is_empty() {
        return uuid::Uuid::new_v4().to_string()[..8].to_string();
    }
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..4])
}

/// Strip block IDs from Markdown content for clean rendering
#[allow(dead_code)]
pub fn strip_block_ids(content: &str) -> String {
    let re = regex::Regex::new(r"<!-- \^[a-f0-9]{8} -->\n?").unwrap();
    re.replace_all(content, "").to_string()
}

/// Extract [[wikilinks]] from text content
#[allow(dead_code)]
pub fn extract_wikilinks(content: &str, source_file: &str) -> Vec<Wikilink> {
    let re = regex::Regex::new(r"\[\[([^\]|#]+)(?:[|#]([^\]]+))?\]\]").unwrap();
    let mut links = Vec::new();

    for cap in re.captures_iter(content) {
        let target = cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let display = cap.get(2).map(|m| m.as_str().to_string());
        let position = cap.get(0).map(|m| m.start()).unwrap_or(0);

        links.push(Wikilink {
            target,
            display,
            source_file: source_file.to_string(),
            position,
        });
    }

    links
}

#[allow(dead_code)]
fn extract_links_from_text(text: &str) -> Vec<String> {
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
    fn test_parse_headings() {
        let content = "# Title\n## Section\nSome text.\n### Subsection\nMore text.";
        let ast = parse_markdown(content);
        assert_eq!(ast.headings.len(), 3); // All headings are captured: # Title, ## Section, ### Subsection
        assert_eq!(ast.blocks.len(), 5); // paragraphs + headings
    }

    #[test]
    fn test_extract_wikilinks() {
        let content = "See [[Rust]] and [[Tauri|the framework]]. Also [[note#heading]].";
        let links = extract_wikilinks(content, "test.md");
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "Rust");
        assert_eq!(links[1].display, Some("the framework".to_string()));
        assert_eq!(links[2].target, "note");
    }

    #[test]
    fn test_block_id_deterministic() {
        let id1 = generate_block_id("Hello world");
        let id2 = generate_block_id("Hello world");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_strip_block_ids() {
        let content = "Some text <!-- ^abc12345 -->\nMore text.";
        let stripped = strip_block_ids(content);
        assert!(!stripped.contains("<!--"));
    }
}
