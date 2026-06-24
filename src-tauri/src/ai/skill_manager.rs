// Skill Manager — Integrates external skills as tools for AI surface
// Wraps deep-research, bayesian-planner, obsidian-markdown, reflection skills
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, PathBuf};

/// Skill type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillType {
    DeepResearch,
    BayesianPlanner,
    ObsidianMarkdown,
    Reflection,
    Custom(String),
}

/// Skill execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub success: bool,
    pub output: String,
    pub artifacts: Vec<Artifact>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Artifact produced by skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub path: String,
    pub artifact_type: ArtifactType,
    pub content_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Spec,
    Plan,
    Cache,
    Reflection,
    Note,
    Report,
}

/// Skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDef {
    pub skill_type: SkillType,
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterDef>,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDef {
    pub name: String,
    pub param_type: ParameterType,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Path,
    Enum(Vec<String>),
}

/// Skill Manager — manages and executes skills
pub struct SkillManager {
    skills: HashMap<String, SkillDef>,
    vault_path: PathBuf,
    project_name: String,
}

impl SkillManager {
    pub fn new(vault_path: PathBuf, project_name: String) -> Self {
        let mut manager = SkillManager {
            skills: HashMap::new(),
            vault_path,
            project_name,
        };
        manager.register_default_skills();
        manager
    }

    fn vault_relative_path(&self, path: &str) -> Result<PathBuf, String> {
        if path.trim().is_empty() {
            return Err("vault-relative path is empty".to_string());
        }

        let mut safe_path = PathBuf::new();
        for component in PathBuf::from(path).components() {
            match component {
                Component::Normal(part) => safe_path.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(format!("unsafe vault-relative path: {}", path));
                }
            }
        }

        if safe_path.as_os_str().is_empty() {
            return Err("vault-relative path is empty".to_string());
        }

        Ok(self.vault_path.join(safe_path))
    }

    /// Register default skills
    fn register_default_skills(&mut self) {
        // Deep Research skill
        self.skills.insert(
            "research".to_string(),
            SkillDef {
                skill_type: SkillType::DeepResearch,
                name: "Deep Research".to_string(),
                description: "Analyze codebase, mine similar projects, generate SPEC".to_string(),
                parameters: vec![
                    ParameterDef {
                        name: "topic".to_string(),
                        param_type: ParameterType::String,
                        description: "Research topic or project path".to_string(),
                        required: true,
                        default: None,
                    },
                    ParameterDef {
                        name: "mode".to_string(),
                        param_type: ParameterType::Enum(vec![
                            "software".to_string(),
                            "academic".to_string(),
                        ]),
                        description: "Research mode".to_string(),
                        required: false,
                        default: Some("software".to_string()),
                    },
                ],
                output_path: "项目流程cache".to_string(),
            },
        );

        // Bayesian Planner skill
        self.skills.insert(
            "plan".to_string(),
            SkillDef {
                skill_type: SkillType::BayesianPlanner,
                name: "Bayesian Planner".to_string(),
                description: "Create and manage Bayesian development plan".to_string(),
                parameters: vec![
                    ParameterDef {
                        name: "action".to_string(),
                        param_type: ParameterType::Enum(vec![
                            "create".to_string(),
                            "execute".to_string(),
                            "fit".to_string(),
                            "validate".to_string(),
                        ]),
                        description: "Plan action".to_string(),
                        required: true,
                        default: None,
                    },
                    ParameterDef {
                        name: "stage".to_string(),
                        param_type: ParameterType::Number,
                        description: "Stage number (for execute/fit/validate)".to_string(),
                        required: false,
                        default: None,
                    },
                ],
                output_path: ".".to_string(),
            },
        );

        // Obsidian Markdown skill
        self.skills.insert(
            "write".to_string(),
            SkillDef {
                skill_type: SkillType::ObsidianMarkdown,
                name: "Obsidian Markdown".to_string(),
                description: "Write and edit Obsidian flavored markdown notes".to_string(),
                parameters: vec![
                    ParameterDef {
                        name: "path".to_string(),
                        param_type: ParameterType::Path,
                        description: "Note path relative to vault".to_string(),
                        required: true,
                        default: None,
                    },
                    ParameterDef {
                        name: "content".to_string(),
                        param_type: ParameterType::String,
                        description: "Note content".to_string(),
                        required: true,
                        default: None,
                    },
                    ParameterDef {
                        name: "frontmatter".to_string(),
                        param_type: ParameterType::Boolean,
                        description: "Include frontmatter".to_string(),
                        required: false,
                        default: Some("true".to_string()),
                    },
                ],
                output_path: ".".to_string(),
            },
        );

        // Reflection skill
        self.skills.insert(
            "reflect".to_string(),
            SkillDef {
                skill_type: SkillType::Reflection,
                name: "Reflection".to_string(),
                description: "Perform project-level reflection and skill improvement".to_string(),
                parameters: vec![ParameterDef {
                    name: "scope".to_string(),
                    param_type: ParameterType::Enum(vec!["light".to_string(), "full".to_string()]),
                    description: "Reflection scope".to_string(),
                    required: false,
                    default: Some("light".to_string()),
                }],
                output_path: "项目流程cache".to_string(),
            },
        );
    }

    /// Execute a skill by name
    pub fn execute_skill(
        &self,
        skill_name: &str,
        params: HashMap<String, String>,
    ) -> Result<SkillResult, String> {
        let skill = self
            .skills
            .get(skill_name)
            .ok_or_else(|| format!("Unknown skill: {}", skill_name))?;

        let start = std::time::Instant::now();

        // Validate required parameters
        for param in &skill.parameters {
            if param.required && !params.contains_key(&param.name) {
                return Ok(SkillResult {
                    success: false,
                    output: String::new(),
                    artifacts: Vec::new(),
                    duration_ms: 0,
                    error: Some(format!("Missing required parameter: {}", param.name)),
                });
            }
        }

        // Execute skill based on type
        let result = match &skill.skill_type {
            SkillType::DeepResearch => self.execute_deep_research(params),
            SkillType::BayesianPlanner => self.execute_bayesian_planner(params),
            SkillType::ObsidianMarkdown => self.execute_obsidian_markdown(params),
            SkillType::Reflection => self.execute_reflection(params),
            SkillType::Custom(name) => Err(format!("Custom skill not implemented: {}", name)),
        };

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok((output, artifacts)) => Ok(SkillResult {
                success: true,
                output,
                artifacts,
                duration_ms: duration,
                error: None,
            }),
            Err(e) => Ok(SkillResult {
                success: false,
                output: String::new(),
                artifacts: Vec::new(),
                duration_ms: duration,
                error: Some(e),
            }),
        }
    }

    /// Execute deep-research skill
    fn execute_deep_research(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let topic = params.get("topic").ok_or("Missing topic parameter")?;

        // Analyze directory structure
        let target_path = self.vault_relative_path(topic)?;
        let scan_path = if target_path.exists() {
            target_path.clone()
        } else {
            self.vault_path.clone()
        };

        let mut file_count = 0;
        let mut dir_count = 0;
        let mut file_list: Vec<String> = Vec::new();

        if scan_path.exists() && scan_path.is_dir() {
            Self::scan_directory(
                &scan_path,
                &mut file_count,
                &mut dir_count,
                &mut file_list,
                0,
                3,
            )?;
        }

        let output = format!(
            "Deep research completed for topic: {}\n\
             Project: {}\n\
             Scanned path: {}\n\
             Files: {}, Directories: {}\n\
             Output: {}/项目流程cache/",
            topic,
            self.project_name,
            scan_path.display(),
            file_count,
            dir_count,
            self.vault_path.display()
        );

        // Generate file list preview
        let preview = file_list
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        let artifacts = vec![Artifact {
            name: "Directory Analysis".to_string(),
            path: format!(
                "{}/项目流程cache/SPEC-{}.md",
                self.vault_path.display(),
                topic
            ),
            artifact_type: ArtifactType::Spec,
            content_preview: Some(preview),
        }];

        Ok((output, artifacts))
    }

    /// Recursively scan directory structure
    fn scan_directory(
        path: &PathBuf,
        file_count: &mut usize,
        dir_count: &mut usize,
        file_list: &mut Vec<String>,
        depth: usize,
        max_depth: usize,
    ) -> Result<(), String> {
        if depth > max_depth {
            return Ok(());
        }

        let entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;

        let indent = "  ".repeat(depth);

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files and common non-essential directories
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }

            if entry_path.is_dir() {
                *dir_count += 1;
                file_list.push(format!("{}📁 {}/", indent, name));
                Self::scan_directory(
                    &entry_path,
                    file_count,
                    dir_count,
                    file_list,
                    depth + 1,
                    max_depth,
                )?;
            } else {
                *file_count += 1;
                if depth <= 2 {
                    file_list.push(format!("{}📄 {}", indent, name));
                }
            }
        }

        Ok(())
    }

    /// Execute bayesian-planner skill
    fn execute_bayesian_planner(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let action = params.get("action").ok_or("Missing action parameter")?;

        let plan_path = self.vault_path.join("bayesian-plan.md");

        match action.as_str() {
            "create" => {
                // Create empty bayesian-plan.md template
                let template = format!(
                    r#"# Bayesian Plan — {project}

## Overview
Project: {project}
Created: {date}

## Stages
<!-- Stages will be added here -->

## Calibration
```json
[]
```

## Reflection Log
```json
[]
```
"#,
                    project = self.project_name,
                    date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );

                if let Some(parent) = plan_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }

                fs::write(&plan_path, template)
                    .map_err(|e| format!("Failed to create plan file: {}", e))?;

                let output = format!("Created bayesian plan template at: {}", plan_path.display());

                let artifacts = vec![Artifact {
                    name: "Bayesian Plan".to_string(),
                    path: plan_path.to_string_lossy().to_string(),
                    artifact_type: ArtifactType::Plan,
                    content_preview: Some("Bayesian development plan template".to_string()),
                }];

                Ok((output, artifacts))
            }
            "read" => {
                // Read existing bayesian-plan.md content
                if !plan_path.exists() {
                    return Err(format!("Plan file not found: {}", plan_path.display()));
                }

                let content = fs::read_to_string(&plan_path)
                    .map_err(|e| format!("Failed to read plan file: {}", e))?;

                let output = format!(
                    "Read bayesian plan from: {}\nContent length: {} chars",
                    plan_path.display(),
                    content.len()
                );

                let artifacts = vec![Artifact {
                    name: "Bayesian Plan".to_string(),
                    path: plan_path.to_string_lossy().to_string(),
                    artifact_type: ArtifactType::Plan,
                    content_preview: Some(content.chars().take(200).collect()),
                }];

                Ok((output, artifacts))
            }
            _ => Err(format!(
                "Unknown action: {}. Supported: create, read",
                action
            )),
        }
    }

    /// Execute obsidian-markdown skill
    fn execute_obsidian_markdown(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let path = params.get("path").ok_or("Missing path parameter")?;
        let content = params.get("content").ok_or("Missing content parameter")?;

        let full_path = self.vault_relative_path(path)?;

        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }

        // Write file content
        fs::write(&full_path, content)
            .map_err(|e| format!("Failed to write file {}: {}", full_path.display(), e))?;

        let output = format!(
            "Note written to: {}\n\
             Content length: {} chars",
            full_path.display(),
            content.len()
        );

        let artifacts = vec![Artifact {
            name: "Note".to_string(),
            path: full_path.to_string_lossy().to_string(),
            artifact_type: ArtifactType::Note,
            content_preview: Some(content.chars().take(100).collect()),
        }];

        Ok((output, artifacts))
    }

    /// Execute reflection skill
    fn execute_reflection(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let scope = params.get("scope").unwrap_or(&"light".to_string()).clone();

        // Scan for REFLECTION-*.md files in 项目流程cache directory
        let cache_path = self.vault_path.join("项目流程cache");
        let mut reflection_files: Vec<String> = Vec::new();

        if cache_path.exists() && cache_path.is_dir() {
            let entries = fs::read_dir(&cache_path)
                .map_err(|e| format!("Failed to read cache directory: {}", e))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let name = entry.file_name().to_string_lossy().to_string();

                if name.starts_with("REFLECTION-") && name.ends_with(".md") {
                    reflection_files.push(name);
                }
            }
        }

        reflection_files.sort();

        let file_list = if reflection_files.is_empty() {
            "No reflection files found".to_string()
        } else {
            reflection_files.join("\n")
        };

        let output = format!(
            "Reflection completed (scope: {})\n\
             Project: {}\n\
             Found {} reflection files in: {}\n\
             {}",
            scope,
            self.project_name,
            reflection_files.len(),
            cache_path.display(),
            file_list
        );

        let artifacts: Vec<Artifact> = reflection_files
            .iter()
            .map(|name| Artifact {
                name: name.clone(),
                path: cache_path.join(name).to_string_lossy().to_string(),
                artifact_type: ArtifactType::Reflection,
                content_preview: None,
            })
            .collect();

        Ok((output, artifacts))
    }

    /// Get list of available skills
    pub fn list_skills(&self) -> Vec<&SkillDef> {
        self.skills.values().collect()
    }

    /// Get skill definition by name
    pub fn get_skill(&self, name: &str) -> Option<&SkillDef> {
        self.skills.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_skill_manager_creation() {
        let manager = SkillManager::new(PathBuf::from("/test/vault"), "test-project".to_string());
        assert_eq!(manager.list_skills().len(), 4);
    }

    #[test]
    fn test_list_skills() {
        let manager = SkillManager::new(PathBuf::from("/test/vault"), "test-project".to_string());
        let skills = manager.list_skills();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Deep Research"));
        assert!(names.contains(&"Bayesian Planner"));
        assert!(names.contains(&"Obsidian Markdown"));
        assert!(names.contains(&"Reflection"));
    }

    #[test]
    fn test_execute_research() {
        let manager = SkillManager::new(PathBuf::from("/test/vault"), "test-project".to_string());
        let mut params = HashMap::new();
        params.insert("topic".to_string(), "test-topic".to_string());

        let result = manager.execute_skill("research", params);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_execute_missing_param() {
        let manager = SkillManager::new(PathBuf::from("/test/vault"), "test-project".to_string());
        let params = HashMap::new();

        let result = manager.execute_skill("research", params);
        assert!(result.is_ok());
        let skill_result = result.unwrap();
        assert!(!skill_result.success);
        assert!(skill_result.error.is_some());
    }

    #[test]
    fn test_execute_unknown_skill() {
        let manager = SkillManager::new(PathBuf::from("/test/vault"), "test-project".to_string());
        let params = HashMap::new();

        let result = manager.execute_skill("unknown", params);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_obsidian_markdown_writes_file() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        let manager = SkillManager::new(vault_path.clone(), "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("path".to_string(), "notes/test-note.md".to_string());
        params.insert(
            "content".to_string(),
            "# Test Note\n\nThis is a test.".to_string(),
        );

        let result = manager.execute_skill("write", params).unwrap();
        assert!(result.success);

        // Verify file was actually written
        let written_path = vault_path.join("notes/test-note.md");
        assert!(written_path.exists());

        let written_content = fs::read_to_string(&written_path).unwrap();
        assert_eq!(written_content, "# Test Note\n\nThis is a test.");
    }

    #[test]
    fn test_execute_obsidian_markdown_creates_parent_dirs() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        let manager = SkillManager::new(vault_path.clone(), "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("path".to_string(), "deep/nested/path/note.md".to_string());
        params.insert("content".to_string(), "content".to_string());

        let result = manager.execute_skill("write", params).unwrap();
        assert!(result.success);

        let written_path = vault_path.join("deep/nested/path/note.md");
        assert!(written_path.exists());
    }

    #[test]
    fn test_execute_obsidian_markdown_rejects_path_escape() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().join("vault");
        fs::create_dir_all(&vault_path).unwrap();

        let manager = SkillManager::new(vault_path.clone(), "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("path".to_string(), "../outside.md".to_string());
        params.insert("content".to_string(), "outside".to_string());

        let result = manager.execute_skill("write", params).unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("unsafe vault-relative path"));
        assert!(!tmp_dir.path().join("outside.md").exists());
    }

    #[test]
    fn test_execute_deep_research_rejects_path_escape_topic() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().join("vault");
        fs::create_dir_all(&vault_path).unwrap();
        fs::create_dir_all(tmp_dir.path().join("outside")).unwrap();

        let manager = SkillManager::new(vault_path, "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("topic".to_string(), "../outside".to_string());

        let result = manager.execute_skill("research", params).unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("unsafe vault-relative path"));
    }

    #[test]
    fn test_execute_bayesian_planner_create() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        let manager = SkillManager::new(vault_path.clone(), "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("action".to_string(), "create".to_string());

        let result = manager.execute_skill("plan", params).unwrap();
        assert!(result.success);

        // Verify plan file was created
        let plan_path = vault_path.join("bayesian-plan.md");
        assert!(plan_path.exists());

        let content = fs::read_to_string(&plan_path).unwrap();
        assert!(content.contains("test-project"));
        assert!(content.contains("Calibration"));
    }

    #[test]
    fn test_execute_bayesian_planner_read() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        // Create a plan file first
        let plan_path = vault_path.join("bayesian-plan.md");
        fs::create_dir_all(plan_path.parent().unwrap()).unwrap();
        fs::write(&plan_path, "# Test Plan\n\nContent here").unwrap();

        let manager = SkillManager::new(vault_path, "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("action".to_string(), "read".to_string());

        let result = manager.execute_skill("plan", params).unwrap();
        assert!(result.success);
        assert!(result.output.contains("Read bayesian plan from"));
        assert!(result.output.contains("Content length:"));
    }

    #[test]
    fn test_execute_bayesian_planner_read_nonexistent() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        let manager = SkillManager::new(vault_path, "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("action".to_string(), "read".to_string());

        let result = manager.execute_skill("plan", params).unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_execute_reflection_scans_files() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        // Create cache directory with reflection files
        let cache_path = vault_path.join("项目流程cache");
        fs::create_dir_all(&cache_path).unwrap();
        fs::write(cache_path.join("REFLECTION-2026-01.md"), "reflection 1").unwrap();
        fs::write(cache_path.join("REFLECTION-2026-02.md"), "reflection 2").unwrap();
        fs::write(cache_path.join("other-file.md"), "not a reflection").unwrap();

        let manager = SkillManager::new(vault_path, "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("scope".to_string(), "light".to_string());

        let result = manager.execute_skill("reflect", params).unwrap();
        assert!(result.success);
        assert!(result.output.contains("2 reflection files"));
        assert_eq!(result.artifacts.len(), 2);
    }

    #[test]
    fn test_execute_deep_research_analyzes_directory() {
        let tmp_dir = TempDir::new().unwrap();
        let vault_path = tmp_dir.path().to_path_buf();

        // Create some test files
        fs::create_dir_all(vault_path.join("src")).unwrap();
        fs::write(vault_path.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(vault_path.join("Cargo.toml"), "[package]").unwrap();

        let manager = SkillManager::new(vault_path, "test-project".to_string());

        let mut params = HashMap::new();
        params.insert("topic".to_string(), "src".to_string());

        let result = manager.execute_skill("research", params).unwrap();
        assert!(result.success);
        assert!(result.output.contains("Files:"));
        assert!(result.output.contains("Directories:"));
    }
}
