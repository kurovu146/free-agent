use std::fs;
use std::path::Path;
use tracing::{info, warn};

/// Load all .md skill files from the skills directory and combine into system prompt
pub fn load_skills(skills_dir: &str) -> String {
    let path = Path::new(skills_dir);
    if !path.exists() {
        warn!("Skills directory not found: {skills_dir}");
        return String::new();
    }

    let mut skills = Vec::new();

    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read skills dir: {e}");
            return String::new();
        }
    };

    for entry in entries.flatten() {
        let file_path = entry.path();
        if file_path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        match fs::read_to_string(&file_path) {
            Ok(content) => {
                skills.push(format!("<!-- skill: {name} -->\n{content}"));
            }
            Err(e) => {
                warn!("Failed to read skill {}: {e}", file_path.display());
            }
        }
    }

    info!("Loaded {} skills", skills.len());
    skills.join("\n\n---\n\n")
}

/// Build the full system prompt from base prompt + skills + memory
pub fn build_system_prompt(base_prompt: &str, skills_content: &str, memory_context: &str) -> String {
    let mut prompt = base_prompt.to_string();

    if !skills_content.is_empty() {
        prompt.push_str("\n\n## Skills\n\n");
        prompt.push_str(skills_content);
    }

    if !memory_context.is_empty() {
        prompt.push_str(memory_context);
    }

    prompt
}
