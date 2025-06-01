use crate::config::Sshconfig;
use crate::zip;
use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn build_project(
    version: &str,
    sess: &ssh2::Session,
    is_history: bool,
    config: &Sshconfig,
) -> Result<()> {
    if !is_history {
        println!("开始构建项目...");

        // 进入前端项目目录
        let home = env::var("HOME")?;
        let project_dir = PathBuf::from(home).join("jobabc/jobabc-internal-admin");
        if !project_dir.exists() {
            anyhow::bail!("项目目录不存在: {}", project_dir.display());
        }
        env::set_current_dir(&project_dir)?;
        println!("当前工作目录: {}", project_dir.display());

        // 检查当前分支
        let current_branch = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .output()?;
        let current_branch = String::from_utf8_lossy(&current_branch.stdout)
            .trim()
            .to_string();

        if current_branch != config.git_checkout {
            anyhow::bail!(
                "当前分支 {} 与配置分支 {} 不匹配",
                current_branch,
                config.git_checkout
            );
        }

        // 检查是否有未提交的修改
        let status = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .output()?;
        if !status.stdout.is_empty() {
            anyhow::bail!("有未提交的修改，请先提交或暂存");
        }

        // 检查是否有未推送的提交
        let ahead = Command::new("git")
            .arg("rev-list")
            .arg("@{u}..HEAD")
            .output()?;
        if !ahead.stdout.is_empty() {
            anyhow::bail!("有未推送的提交，请先推送");
        }

        // 执行构建
        println!("构建项目...");
        let status = Command::new("pnpm")
            .arg("run")
            .arg(&config.build)
            .status()?;

        if !status.success() {
            anyhow::bail!("构建失败");
        }

        println!("构建完成，版本: {}", version);
    }

    // 压缩和部署
    zip::compress_and_deploy(version, sess, is_history, &config)?;

    Ok(())
}
