// 版本管理

use anyhow::Result;
use dialoguer::{Select, theme::ColorfulTheme};
use std::cmp::Ordering;

#[derive(Debug)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim_start_matches("v").trim_end_matches(".zip");
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;

        Some(Version {
            major,
            minor,
            patch,
        })
    }

    pub fn to_string(&self) -> String {
        format!("v{}.{}.{}.zip", self.major, self.minor, self.patch)
    }

    pub fn increment(&self) -> Self {
        let mut new_version = self.clone();
        new_version.patch += 1;
        if new_version.patch > 99 {
            new_version.patch = 0;
            new_version.minor += 1;
            if new_version.minor > 99 {
                new_version.minor = 0;
                new_version.major += 1;
            }
        }
        new_version
    }
}

impl Clone for Version {
    fn clone(&self) -> Self {
        Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
        }
    }
}

pub fn get_latest_version(history: &[String]) -> Option<Version> {
    history
        .iter()
        .filter_map(|s| Version::from_str(s))
        .max_by(|a, b| match a.major.cmp(&b.major) {
            Ordering::Equal => match a.minor.cmp(&b.minor) {
                Ordering::Equal => a.patch.cmp(&b.patch),
                other => other,
            },
            other => other,
        })
}

pub fn show_version_menu() -> Result<String> {
    let options = vec!["版本自增", "指定版本", "历史版本"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("请选择版本管理方式")
        .items(&options)
        .default(0)
        .interact()?;

    Ok(options[selection].to_string())
}

pub fn select_history_version(history: &[String]) -> Result<Option<String>> {
    if history.is_empty() {
        println!("没有历史版本");
        return Ok(None);
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("请选择历史版本")
        .items(history)
        .default(0)
        .interact()?;

    Ok(Some(history[selection].clone()))
}

pub fn validate_version(version: &str) -> bool {
    Version::from_str(version).is_some()
}
