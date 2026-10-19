//! Optional site features shared by the build, preview and validation commands.

use crate::references::read_utf8_sig;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn read_config(site: &Path) -> Result<toml::Table> {
    Ok(read_utf8_sig(&site.join("config.toml"))?.parse::<toml::Table>()?)
}

/// A new site can build without personal data or a Typst resume.
pub fn profile_enabled(config: &toml::Table) -> Result<bool> {
    let value = config
        .get("extra")
        .and_then(toml::Value::as_table)
        .and_then(|extra| extra.get("profile_enabled"));
    match value {
        None => Ok(false),
        Some(toml::Value::Boolean(enabled)) => Ok(*enabled),
        Some(_) => bail!("extra.profile_enabled must be true or false"),
    }
}

pub fn require_profile_source(root: &Path) -> Result<PathBuf> {
    let source = root.join("profile").join("cv.typ");
    if !source.is_file() {
        bail!(
            "Profile is enabled but profile/cv.typ is missing. Add a profile source \
             or set extra.profile_enabled = false in site/config.toml."
        );
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn profile_is_off_by_default_and_must_be_boolean() {
        assert!(!profile_enabled(&config("base_url = \"x\"")).unwrap());
        assert!(profile_enabled(&config("[extra]\nprofile_enabled = true")).unwrap());
        assert!(!profile_enabled(&config("[extra]\nprofile_enabled = false")).unwrap());
        let error = profile_enabled(&config("[extra]\nprofile_enabled = \"yes\"")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "extra.profile_enabled must be true or false"
        );
    }

    #[test]
    fn missing_profile_source_is_reported() {
        let root = std::env::temp_dir().join(format!("site-test-{}", std::process::id()));
        let error = require_profile_source(&root).unwrap_err().to_string();
        assert!(error.contains("profile/cv.typ is missing"), "{error}");
    }

    #[test]
    fn config_accepts_a_byte_order_mark() {
        let root = std::env::temp_dir().join(format!("site-config-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("config.toml"),
            "\u{feff}base_url = \"https://e.org/\"\n",
        )
        .unwrap();
        assert_eq!(
            read_config(&root).unwrap()["base_url"].as_str(),
            Some("https://e.org/")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
