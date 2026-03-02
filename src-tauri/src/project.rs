use std::path::PathBuf;

/// Compute the Claude project directory path for a given workspace path.
/// Mirrors the logic in Claude Code: characters that are not alphanumeric
/// or '-' are replaced with '-'.
pub fn get_project_dir(workspace_path: &str) -> PathBuf {
    let dir_name: String = workspace_path
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".claude")
        .join("projects")
        .join(dir_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_dir() {
        let path = get_project_dir("/Users/ggm/work/project");
        let dir_name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(dir_name, "-Users-ggm-work-project");
    }

    #[test]
    fn test_underscores_and_dots_replaced() {
        let path = get_project_dir("/Users/test/session_123/.claude/worktrees/comwit");
        let dir_name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(dir_name, "-Users-test-session-123--claude-worktrees-comwit");
    }
}
