// Block ID utilities — Generate and manage content-based block identifiers

use sha2::{Sha256, Digest};

/// Generate a deterministic block ID from content text
pub fn generate_block_id(text: &str) -> String {
    if text.trim().is_empty() {
        return uuid::Uuid::new_v4().to_string()[..8].to_string();
    }
    let mut hasher = Sha256::new();
    hasher.update(text.trim().as_bytes());
    hex::encode(&hasher.finalize()[..4])
}

/// Strip invisible block ID markers from Markdown for clean rendering
pub fn strip_block_ids(content: &str) -> String {
    // Remove <!--^...--> markers
    let mut result = String::new();
    let mut in_block_id = false;
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' && chars.peek() == Some(&'!') {
            let mut lookahead = String::new();
            lookahead.push(ch);
            while let Some(&next) = chars.peek() {
                lookahead.push(next);
                chars.next();
                if next == '>' && lookahead.contains("<!--^") {
                    in_block_id = true;
                    break;
                } else if next == '>' {
                    break;
                }
            }
            if in_block_id {
                in_block_id = false;
                // Skip trailing newline if present
                if chars.peek() == Some(&'\n') { chars.next(); }
                if chars.peek() == Some(&'\r') { chars.next(); }
                continue;
            }
        }
        result.push(ch);
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_block_ids() {
        let content = "Some paragraph\n<!--^abc12345-->\nAnother paragraph";
        let stripped = strip_block_ids(content);
        assert_eq!(stripped, "Some paragraph\nAnother paragraph");
    }

    #[test]
    fn test_generate_block_id_deterministic() {
        let id1 = generate_block_id("Hello");
        let id2 = generate_block_id("Hello");
        assert_eq!(id1, id2);
    }
}
