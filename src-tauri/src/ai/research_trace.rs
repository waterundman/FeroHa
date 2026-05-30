use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathLogEntry {
    pub hop: u32,
    pub query: String,
    pub source: String,
    pub urls: Vec<String>,
    pub excluded: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSummary {
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub total_blocks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContext {
    pub card_id: Option<String>,
    pub card_type: Option<String>,
    pub intent: String,
    pub ghost_ids: Vec<String>,
    pub feedback_summary: Option<FeedbackSummary>,
    pub phase_timings: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTrace {
    pub path_log: Vec<PathLogEntry>,
    pub cot_log: String,
    pub result: String,
    pub context: Option<TaskContext>,
}

#[allow(clippy::too_many_arguments)]
pub fn write_path_log(
    dualtrack_dir: &Path,
    task_id: &str,
    hop: u32,
    query: &str,
    source: &str,
    urls: &[String],
    excluded: &[String],
    reason: &str,
    context: Option<&TaskContext>,
) -> io::Result<()> {
    let dir = dualtrack_dir
        .join(".dualtrack/research/paths")
        .join(task_id);
    fs::create_dir_all(&dir)?;
    let file_path = dir.join("path_log.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)?;
    let entry = PathLogEntry {
        hop,
        query: query.to_string(),
        source: source.to_string(),
        urls: urls.to_vec(),
        excluded: excluded.to_vec(),
        reason: reason.to_string(),
    };
    let line = serde_json::to_string(&entry).map_err(io::Error::other)?;
    writeln!(file, "{}", line)?;
    let _ = context;
    file.flush()?;
    Ok(())
}

pub fn write_cot_log(
    dualtrack_dir: &Path,
    task_id: &str,
    content: &str,
    context: Option<&TaskContext>,
) -> io::Result<()> {
    let dir = dualtrack_dir
        .join(".dualtrack/research/paths")
        .join(task_id);
    fs::create_dir_all(&dir)?;
    let file_path = dir.join("cot_log.md");
    let final_content = if let Some(ctx) = context {
        if !ctx.intent.is_empty() {
            format!("# Intent: {}\n\n{}", ctx.intent, content)
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    };
    fs::write(&file_path, &final_content)?;
    Ok(())
}

pub fn write_result_md(
    dualtrack_dir: &Path,
    task_id: &str,
    content: &str,
    context: Option<&TaskContext>,
) -> io::Result<()> {
    let dir = dualtrack_dir
        .join(".dualtrack/research/results")
        .join(task_id);
    fs::create_dir_all(&dir)?;
    let file_path = dir.join("result.md");

    let has_citations = content.contains("## Citations") || content.contains("## 引用");
    let has_excluded = content.contains("## Excluded Sources") || content.contains("## 排除来源");

    let mut final_content = String::new();

    if let Some(ctx) = context {
        final_content.push_str("---\n");
        final_content.push_str(&format!(
            "card_id: {}\n",
            ctx.card_id.as_deref().unwrap_or("null")
        ));
        final_content.push_str(&format!(
            "card_type: {}\n",
            ctx.card_type.as_deref().unwrap_or("null")
        ));
        final_content.push_str(&format!(
            "intent: \"{}\"\n",
            ctx.intent.replace('"', "\\\"")
        ));
        final_content.push_str("ghost_ids:\n");
        for gid in &ctx.ghost_ids {
            final_content.push_str(&format!("  - {}\n", gid));
        }
        if let Some(ref fs) = ctx.feedback_summary {
            final_content.push_str("feedback_summary:\n");
            final_content.push_str(&format!("  accepted_count: {}\n", fs.accepted_count));
            final_content.push_str(&format!("  rejected_count: {}\n", fs.rejected_count));
            final_content.push_str(&format!("  total_blocks: {}\n", fs.total_blocks));
        } else {
            final_content.push_str("feedback_summary: null\n");
        }
        final_content.push_str("phase_timings:\n");
        for (key, val) in &ctx.phase_timings {
            final_content.push_str(&format!("  {}: {}\n", key, val));
        }
        final_content.push_str("---\n\n");
    }

    if !has_citations {
        final_content.push_str("## Citations\n\nN/A\n\n");
    }
    final_content.push_str(content);
    if !has_excluded {
        if !final_content.ends_with('\n') {
            final_content.push('\n');
        }
        final_content.push_str("\n## Excluded Sources\n\nN/A\n");
    }

    fs::write(&file_path, &final_content)?;
    Ok(())
}

pub fn write_context(dualtrack_dir: &Path, task_id: &str, context: &TaskContext) -> io::Result<()> {
    let dir = dualtrack_dir
        .join(".dualtrack/research/results")
        .join(task_id);
    fs::create_dir_all(&dir)?;
    let file_path = dir.join("context.json");
    let json = serde_json::to_string_pretty(context).map_err(io::Error::other)?;
    fs::write(&file_path, json)?;
    Ok(())
}

pub fn get_task_trace(dualtrack_dir: &Path, task_id: &str) -> Result<TaskTrace, String> {
    let path_dir = dualtrack_dir
        .join(".dualtrack/research/paths")
        .join(task_id);
    let path_log_path = path_dir.join("path_log.jsonl");

    let raw_log = fs::read_to_string(&path_log_path)
        .map_err(|_| format!("path_log.jsonl not found for task {}", task_id))?;

    let mut path_log = Vec::new();
    for line in raw_log.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: PathLogEntry = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse path_log line: {}", e))?;
        path_log.push(entry);
    }

    let cot_log_path = path_dir.join("cot_log.md");
    let cot_log = fs::read_to_string(&cot_log_path)
        .map_err(|e| format!("Failed to read cot_log.md: {}", e))?;

    let result_dir = dualtrack_dir
        .join(".dualtrack/research/results")
        .join(task_id);
    let result_path = result_dir.join("result.md");
    let result =
        fs::read_to_string(&result_path).map_err(|e| format!("Failed to read result.md: {}", e))?;

    let context_path = result_dir.join("context.json");
    let context = if context_path.exists() {
        let raw = fs::read_to_string(&context_path)
            .map_err(|e| format!("Failed to read context.json: {}", e))?;
        Some(
            serde_json::from_str(&raw)
                .map_err(|e| format!("Failed to parse context.json: {}", e))?,
        )
    } else {
        None
    };

    Ok(TaskTrace {
        path_log,
        cot_log,
        result,
        context,
    })
}

pub fn has_trace(dualtrack_dir: &Path, task_id: &str) -> bool {
    let path_log_path = dualtrack_dir
        .join(".dualtrack/research/paths")
        .join(task_id)
        .join("path_log.jsonl");
    path_log_path.exists()
}
