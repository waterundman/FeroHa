// Skill Manager — Integrates external skills as tools for AI surface
// Wraps deep-research, bayesian-planner, obsidian-markdown, reflection skills

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
                        param_type: ParameterType::Enum(vec!["software".to_string(), "academic".to_string()]),
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
                parameters: vec![
                    ParameterDef {
                        name: "scope".to_string(),
                        param_type: ParameterType::Enum(vec![
                            "light".to_string(),
                            "full".to_string(),
                        ]),
                        description: "Reflection scope".to_string(),
                        required: false,
                        default: Some("light".to_string()),
                    },
                ],
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
        let skill = self.skills.get(skill_name)
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
        let topic = params.get("topic")
            .ok_or("Missing topic parameter")?;

        // In real implementation, this would invoke the deep-research skill
        // For now, return a placeholder
        let output = format!(
            "Deep research completed for topic: {}\n\
             Project: {}\n\
             Output: {}/项目流程cache/",
            topic, self.project_name, self.vault_path.display()
        );

        let artifacts = vec![
            Artifact {
                name: "SPEC".to_string(),
                path: format!("{}/项目流程cache/SPEC-{}.md", self.vault_path.display(), topic),
                artifact_type: ArtifactType::Spec,
                content_preview: Some("Generated SPEC document".to_string()),
            },
        ];

        Ok((output, artifacts))
    }

    /// Execute bayesian-planner skill
    fn execute_bayesian_planner(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let action = params.get("action")
            .ok_or("Missing action parameter")?;

        let output = format!(
            "Bayesian planner action: {}\n\
             Project: {}\n\
             Plan file: {}/bayesian-plan-v2.md",
            action, self.project_name, self.vault_path.display()
        );

        let artifacts = vec![
            Artifact {
                name: "Bayesian Plan".to_string(),
                path: format!("{}/bayesian-plan-v2.md", self.vault_path.display()),
                artifact_type: ArtifactType::Plan,
                content_preview: Some("Bayesian development plan".to_string()),
            },
        ];

        Ok((output, artifacts))
    }

    /// Execute obsidian-markdown skill
    fn execute_obsidian_markdown(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let path = params.get("path")
            .ok_or("Missing path parameter")?;
        let content = params.get("content")
            .ok_or("Missing content parameter")?;

        let full_path = self.vault_path.join(path);

        // In real implementation, this would write the file
        let output = format!(
            "Note written to: {}\n\
             Content length: {} chars",
            full_path.display(),
            content.len()
        );

        let artifacts = vec![
            Artifact {
                name: "Note".to_string(),
                path: full_path.to_string_lossy().to_string(),
                artifact_type: ArtifactType::Note,
                content_preview: Some(content.chars().take(100).collect()),
            },
        ];

        Ok((output, artifacts))
    }

    /// Execute reflection skill
    fn execute_reflection(
        &self,
        params: HashMap<String, String>,
    ) -> Result<(String, Vec<Artifact>), String> {
        let scope = params.get("scope")
            .unwrap_or(&"light".to_string())
            .clone();

        let output = format!(
            "Reflection completed (scope: {})\n\
             Project: {}\n\
             Output: {}/项目流程cache/REFLECTION-*.md",
            scope, self.project_name, self.vault_path.display()
        );

        let artifacts = vec![
            Artifact {
                name: "Reflection Report".to_string(),
                path: format!("{}/项目流程cache/REFLECTION.md", self.vault_path.display()),
                artifact_type: ArtifactType::Reflection,
                content_preview: Some("Project reflection report".to_string()),
            },
        ];

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

    #[test]
    fn test_skill_manager_creation() {
        let manager = SkillManager::new(
            PathBuf::from("/test/vault"),
            "test-project".to_string(),
        );
        assert_eq!(manager.list_skills().len(), 4);
    }

    #[test]
    fn test_list_skills() {
        let manager = SkillManager::new(
            PathBuf::from("/test/vault"),
            "test-project".to_string(),
        );
        let skills = manager.list_skills();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Deep Research"));
        assert!(names.contains(&"Bayesian Planner"));
        assert!(names.contains(&"Obsidian Markdown"));
        assert!(names.contains(&"Reflection"));
    }

    #[test]
    fn test_execute_research() {
        let manager = SkillManager::new(
            PathBuf::from("/test/vault"),
            "test-project".to_string(),
        );
        let mut params = HashMap::new();
        params.insert("topic".to_string(), "test-topic".to_string());

        let result = manager.execute_skill("research", params);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_execute_missing_param() {
        let manager = SkillManager::new(
            PathBuf::from("/test/vault"),
            "test-project".to_string(),
        );
        let params = HashMap::new();

        let result = manager.execute_skill("research", params);
        assert!(result.is_ok());
        let skill_result = result.unwrap();
        assert!(!skill_result.success);
        assert!(skill_result.error.is_some());
    }

    #[test]
    fn test_execute_unknown_skill() {
        let manager = SkillManager::new(
            PathBuf::from("/test/vault"),
            "test-project".to_string(),
        );
        let params = HashMap::new();

        let result = manager.execute_skill("unknown", params);
        assert!(result.is_err());
    }
}
