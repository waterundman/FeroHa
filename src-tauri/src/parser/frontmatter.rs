// YAML Frontmatter Parser — Extracts metadata from Markdown frontmatter blocks

use serde::{Deserialize, Serialize};

use crate::mdt::types::MdtMeta;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    #[serde(flatten)]
    pub mdt: Option<MdtMeta>,
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct TagVisitor;

    impl<'de> Visitor<'de> for TagVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a list of strings or a comma-separated string")
        }

        fn visit_str<E>(self, s: &str) -> Result<Vec<String>, E>
        where
            E: de::Error,
        {
            Ok(s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut tags = Vec::new();
            while let Some(tag) = seq.next_element::<String>()? {
                tags.push(tag);
            }
            Ok(tags)
        }
    }

    deserializer.deserialize_any(TagVisitor)
}

/// Extract #tag style inline tags from note body content.
/// `body_offset` is the byte index where body content begins (after frontmatter).
pub fn extract_inline_tags(content: &str, body_offset: usize) -> Vec<String> {
    let body = &content[body_offset..];
    let mut tags = Vec::new();

    let code_block_pattern = regex::Regex::new(r"```[\s\S]*?```").unwrap();
    let cleaned = code_block_pattern.replace_all(body, "");

    let tag_pattern =
        regex::Regex::new(r"(?:^|\s)#([a-zA-Z\u{4e00}-\u{9fff}][\w\u{4e00}-\u{9fff}/-]*)").unwrap();
    for cap in tag_pattern.captures_iter(&cleaned) {
        let tag = cap[1].to_lowercase();
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

/// Parse YAML frontmatter from Markdown content.
///
/// Returns `Some((frontmatter, body_start_offset))` if frontmatter exists and parses
/// successfully. `body_start_offset` is the byte index where body content begins.
/// Returns `None` if no valid frontmatter is found.
pub fn parse_frontmatter(content: &str) -> Option<(Frontmatter, usize)> {
    if !content.starts_with("---") {
        return None;
    }

    let after_open = &content[3..];
    let closing_marker = after_open.find("\n---")?;

    let yaml_str = &after_open[..closing_marker];
    let yaml_str = yaml_str.trim();

    let frontmatter: Frontmatter = if yaml_str.is_empty() {
        Frontmatter::default()
    } else {
        serde_yaml::from_str(yaml_str).ok()?
    };

    let after_close = &after_open[closing_marker..];
    let after_close = &after_close[1..]; // skip \n
    let after_close = &after_close[3..]; // skip ---
    let body = after_close.trim_start_matches(['\n', '\r']);
    let body_offset = body.as_ptr() as usize - content.as_ptr() as usize;

    Some((frontmatter, body_offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_frontmatter() {
        assert!(parse_frontmatter("# Just a heading").is_none());
    }

    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\nbody text";
        let (fm, offset) = parse_frontmatter(content).unwrap();
        assert!(fm.title.is_none());
        assert!(fm.tags.is_empty());
        assert_eq!(&content[offset..], "body text");
    }

    #[test]
    fn test_frontmatter_with_tags_list() {
        let content = "---\ntitle: Hello\ntags:\n  - rust\n  - tauri\n---\nbody text";
        let (fm, offset) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title.as_deref(), Some("Hello"));
        assert_eq!(fm.tags, vec!["rust", "tauri"]);
        assert_eq!(&content[offset..], "body text");
    }

    #[test]
    fn test_frontmatter_with_tags_string() {
        let content = "---\ntags: rust, tauri, wasm\n---\nbody text";
        let (fm, offset) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["rust", "tauri", "wasm"]);
        assert_eq!(&content[offset..], "body text");
    }

    #[test]
    fn test_frontmatter_with_aliases() {
        let content = "---\naliases:\n  - my-note\n  - other-name\n---\nbody text";
        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(
            fm.aliases,
            Some(vec!["my-note".to_string(), "other-name".to_string()])
        );
    }

    #[test]
    fn test_parse_mdt_frontmatter_fields() {
        let content = "---\nmdt_version: \"0.1.0\"\nid: \"node-1\"\ntitle: MDT Node\ntree:\n  parent: null\n  order: 3\n  path: [root, design]\n  depth: 1\narea: memory-design\nimportance: 4\nsummary: \"A node summary\"\nlinks:\n  - target: \"node-2\"\n    type: reference\nstorage:\n  tier: warm\n  pinned: false\n---\n# Body\n";
        let (fm, offset) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title.as_deref(), Some("MDT Node"));
        assert_eq!(fm.mdt.as_ref().unwrap().id.as_deref(), Some("node-1"));
        assert_eq!(fm.mdt.as_ref().unwrap().tree.as_ref().unwrap().order, 3);
        assert_eq!(fm.mdt.as_ref().unwrap().links[0].edge_type, "reference");
        assert_eq!(&content[offset..], "# Body\n");
    }

    #[test]
    fn test_invalid_yaml_returns_none() {
        let content = "---\ninvalid: [unclosed\n---\nbody";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_body_offset_correct() {
        let content = "---\ntitle: Test\n---\n# Heading\n\nSome text.\n";
        let (_, offset) = parse_frontmatter(content).unwrap();
        assert_eq!(&content[offset..], "# Heading\n\nSome text.\n");
    }
}
