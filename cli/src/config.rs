use anyhow::{Context, Result};
use statichub_shared::ProjectConfig;
use std::path::{Path, PathBuf};

pub fn find_config_file(dir: &Path) -> Option<PathBuf> {
    let yaml_path = dir.join("statichub.yaml");
    if yaml_path.exists() {
        return Some(yaml_path);
    }

    let yml_path = dir.join("statichub.yml");
    if yml_path.exists() {
        return Some(yml_path);
    }

    None
}

pub fn load_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let config: ProjectConfig = serde_yaml::from_str(&content)
        .with_context(|| "Failed to parse config file")?;

    Ok(config)
}

pub fn merge_config(base: ProjectConfig, overrides: ProjectConfig) -> ProjectConfig {
    ProjectConfig {
        name: overrides.name.or(base.name),
        clean_urls: overrides.clean_urls.or(base.clean_urls),
        spa: overrides.spa.or(base.spa),
        headers: overrides.headers.or(base.headers),
        redirects: overrides.redirects.or(base.redirects),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_config_file_yaml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("statichub.yaml");
        fs::write(&config_path, "name: test").unwrap();

        let found = find_config_file(temp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), config_path);
    }

    #[test]
    fn test_load_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("statichub.yaml");
        fs::write(&config_path, "name: my-project\nclean_urls: true").unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.name, Some("my-project".to_string()));
        assert_eq!(config.clean_urls, Some(true));
    }

    #[test]
    fn test_merge_config() {
        let base = ProjectConfig {
            name: Some("base".to_string()),
            clean_urls: Some(false),
            spa: None,
            headers: None,
            redirects: None,
        };

        let overrides = ProjectConfig {
            name: None,
            clean_urls: Some(true),
            spa: Some(true),
            headers: None,
            redirects: None,
        };

        let merged = merge_config(base, overrides);
        assert_eq!(merged.name, Some("base".to_string()));
        assert_eq!(merged.clean_urls, Some(true));
        assert_eq!(merged.spa, Some(true));
    }
}
