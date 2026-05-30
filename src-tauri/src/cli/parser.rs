// CLI Command Parser — Parse /agent and other slash commands

use serde::{Deserialize, Serialize};

/// All supported CLI commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", content = "params")]
pub enum CliCommand {
    /// /agent search --query "..." [--top-k N]
    Search {
        query: String,
        #[serde(default = "default_top_k")]
        top_k: usize,
    },
    /// /agent summarize --target [[Note]] [--style bullet|paragraph]
    Summarize {
        target: String,
        #[serde(default = "default_style")]
        style: SummarizeStyle,
    },
    /// /agent fetch-papers --topic "..." [--max N] [--link-to [[Note]]]
    FetchPapers {
        topic: String,
        #[serde(default = "default_max_papers")]
        max: usize,
        link_to: Option<String>,
    },
    /// /agent deep-dive [[Concept]] [--depth N]
    DeepDive {
        concept: String,
        #[serde(default = "default_depth")]
        depth: usize,
    },
    /// /agent explain [[Concept]] [--level beginner|intermediate|expert]
    Explain {
        concept: String,
        #[serde(default = "default_level")]
        level: ExplainLevel,
    },
    /// /agent diff --review (show pending AI suggestions)
    DiffReview,
    /// /agent status (show running/queued agent tasks)
    Status,
    /// /agent config --model <model> (set LLM model)
    Config { model: Option<String> },
    /// /agent dream (run memory consolidation cycle)
    Dream,
    /// /agent research <question> [--max-iterations N]
    /// /agent deep-research <question> [--max-iterations N]
    DeepResearch {
        question: String,
        max_iterations: Option<usize>,
    },
    /// Custom command dispatched programmatically (not via CLI parsing)
    Custom(String),
    /// CommandCard pipeline command (dispatched via frontend CommandCard pipeline)
    CustomCard {
        prompt: String,
        params: Option<std::collections::HashMap<String, String>>,
        card_type: Option<String>,
        card_id: Option<String>,
    },
}

fn default_top_k() -> usize {
    5
}
fn default_max_papers() -> usize {
    5
}
fn default_depth() -> usize {
    2
}

fn default_style() -> SummarizeStyle {
    SummarizeStyle::Bullet
}
fn default_level() -> ExplainLevel {
    ExplainLevel::Intermediate
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummarizeStyle {
    Bullet,
    Paragraph,
    Outline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExplainLevel {
    Beginner,
    Intermediate,
    Expert,
}

#[derive(Debug, thiserror::Error)]
pub enum CliParseError {
    #[error("Unknown command: {0}")]
    UnknownCommand(String),
    #[error("Missing required parameter: {0}")]
    MissingParam(String),
    #[error("Invalid parameter value: {0}")]
    #[allow(dead_code)]
    InvalidParam(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Parse a CLI input string into a typed command
///
/// Grammar:
///   /agent <verb> [--flag1 value1] [--flag2=value2] [--positional]
///
/// Examples:
///   /agent search --query "Rust async patterns" --top-k 10
///   /agent summarize --target [[My Note]]
///   /agent deep-dive [[Rust/lifetimes]] --depth 3
pub fn parse(input: &str) -> Result<CliCommand, CliParseError> {
    let input = input.trim();

    // Must start with /agent or /
    let (verb, args_str) = if let Some(rest) = input.strip_prefix("/agent ") {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let verb = parts[0];
        let args = if parts.len() > 1 { parts[1] } else { "" };
        (verb, args)
    } else if let Some(rest) = input.strip_prefix('/') {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let verb = parts[0];
        let args = if parts.len() > 1 { parts[1] } else { "" };
        (verb, args)
    } else {
        return Err(CliParseError::ParseError(
            "Command must start with /agent or /".to_string(),
        ));
    };

    let args = parse_args(args_str);

    match verb {
        "search" => Ok(CliCommand::Search {
            query: args
                .get_str("query", "q")
                .ok_or_else(|| CliParseError::MissingParam("--query".to_string()))?,
            top_k: args.get_usize("top-k", "k").unwrap_or(5),
        }),
        "summarize" => Ok(CliCommand::Summarize {
            target: args.get_str("target", "t").unwrap_or_default(),
            style: args
                .get_str("style", "s")
                .map(|s| match s.to_lowercase().as_str() {
                    "paragraph" | "p" => SummarizeStyle::Paragraph,
                    "outline" | "o" => SummarizeStyle::Outline,
                    _ => SummarizeStyle::Bullet,
                })
                .unwrap_or(SummarizeStyle::Bullet),
        }),
        "fetch-papers" | "fetch_papers" | "fetch" => Ok(CliCommand::FetchPapers {
            topic: args
                .get_str("topic", "t")
                .ok_or_else(|| CliParseError::MissingParam("--topic".to_string()))?,
            max: args.get_usize("max", "m").unwrap_or(5),
            link_to: args.get_str("link-to", "l"),
        }),
        "deep-dive" | "deep_dive" | "deepdive" => {
            // Try positional arg first, then --concept
            let concept = args
                .positional()
                .or_else(|| args.get_str("concept", "c"))
                .unwrap_or_default();
            Ok(CliCommand::DeepDive {
                concept,
                depth: args.get_usize("depth", "d").unwrap_or(2),
            })
        }
        "explain" => {
            let concept = args
                .positional()
                .or_else(|| args.get_str("concept", "c"))
                .unwrap_or_default();
            Ok(CliCommand::Explain {
                concept,
                level: args
                    .get_str("level", "l")
                    .map(|s| match s.to_lowercase().as_str() {
                        "beginner" | "b" => ExplainLevel::Beginner,
                        "expert" | "e" => ExplainLevel::Expert,
                        _ => ExplainLevel::Intermediate,
                    })
                    .unwrap_or(ExplainLevel::Intermediate),
            })
        }
        "diff" | "review" => Ok(CliCommand::DiffReview),
        "status" | "st" => Ok(CliCommand::Status),
        "config" | "cfg" => Ok(CliCommand::Config {
            model: args.get_str("model", "m"),
        }),
        "research" | "deep-research" | "deep_research" => {
            let question = args
                .positional()
                .or_else(|| args.get_str("question", "q"))
                .ok_or_else(|| CliParseError::MissingParam("question".to_string()))?;
            Ok(CliCommand::DeepResearch {
                question,
                max_iterations: args.get_usize("max-iterations", "m"),
            })
        }
        "dream" => Ok(CliCommand::Dream),
        _ => Err(CliParseError::UnknownCommand(verb.to_string())),
    }
}

/// Parsed argument map with support for --flag value, --flag=value, and [[positional]]
struct ArgsMap {
    flags: Vec<(String, String)>,
    positional: Vec<String>,
}

impl ArgsMap {
    fn get_str(&self, long: &str, short: &str) -> Option<String> {
        self.flags
            .iter()
            .find(|(k, _)| k == long || k == short)
            .map(|(_, v)| v.clone())
            .or_else(|| self.positional.first().cloned())
    }

    fn get_usize(&self, long: &str, short: &str) -> Option<usize> {
        self.get_str(long, short)
            .and_then(|v| v.parse::<usize>().ok())
    }

    fn positional(&self) -> Option<String> {
        self.positional.first().cloned()
    }
}

fn parse_args(input: &str) -> ArgsMap {
    let mut flags = Vec::new();
    let mut positional = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();

    while i < chars.len() {
        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        if chars[i] == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            // --flag or --flag=value or --flag value
            i += 2;
            let mut flag = String::new();
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' {
                flag.push(chars[i]);
                i += 1;
            }

            let value = if i < chars.len() && chars[i] == '=' {
                i += 1;
                if i < chars.len() && chars[i] == '"' {
                    i += 1;
                    let mut val = String::new();
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 1;
                            val.push(chars[i]);
                        } else {
                            val.push(chars[i]);
                        }
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1;
                    } // skip closing "
                    val
                } else {
                    let mut val = String::new();
                    while i < chars.len() && !chars[i].is_whitespace() {
                        val.push(chars[i]);
                        i += 1;
                    }
                    val
                }
            } else {
                // --flag value
                let mut val = String::new();
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '"' {
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 1;
                            val.push(chars[i]);
                        } else {
                            val.push(chars[i]);
                        }
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1;
                    }
                } else {
                    while i < chars.len() && !chars[i].is_whitespace() {
                        val.push(chars[i]);
                        i += 1;
                    }
                }
                val
            };

            flags.push((flag, value));
        } else if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // [[wikilink]]
            i += 2;
            let mut target = String::new();
            while i < chars.len() && chars[i] != ']' {
                target.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            } // skip first ]
            if i < chars.len() && chars[i] == ']' {
                i += 1;
            } // skip second ]
            positional.push(target);
        } else if chars[i] == '"' {
            // Quoted string
            i += 1;
            let mut val = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    val.push(chars[i]);
                } else {
                    val.push(chars[i]);
                }
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            positional.push(val);
        } else {
            // Bare word
            let mut word = String::new();
            while i < chars.len() && !chars[i].is_whitespace() {
                word.push(chars[i]);
                i += 1;
            }
            positional.push(word);
        }
    }

    ArgsMap { flags, positional }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search() {
        let cmd = parse(r#"/agent search --query "Rust async""#).unwrap();
        match cmd {
            CliCommand::Search { query, top_k } => {
                assert_eq!(query, "Rust async");
                assert_eq!(top_k, 5);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_parse_deep_dive() {
        let cmd = parse(r#"/agent deep-dive [[Rust/lifetimes]] --depth 3"#).unwrap();
        match cmd {
            CliCommand::DeepDive { concept, depth } => {
                assert_eq!(concept, "Rust/lifetimes");
                assert_eq!(depth, 3);
            }
            _ => panic!("Expected DeepDive command"),
        }
    }

    #[test]
    fn test_parse_summarize_with_style() {
        let cmd = parse(r#"/agent summarize --target "My Note" --style paragraph"#).unwrap();
        match cmd {
            CliCommand::Summarize { target, style } => {
                assert_eq!(target, "My Note");
                assert!(matches!(style, SummarizeStyle::Paragraph));
            }
            _ => panic!("Expected Summarize command"),
        }
    }

    #[test]
    fn test_parse_unknown_verb() {
        let result = parse("/agent foobar");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_status() {
        let cmd = parse("/agent status").unwrap();
        assert!(matches!(cmd, CliCommand::Status));
    }

    #[test]
    fn test_parse_dream() {
        let cmd = parse("/agent dream").unwrap();
        assert!(matches!(cmd, CliCommand::Dream));
    }
}
