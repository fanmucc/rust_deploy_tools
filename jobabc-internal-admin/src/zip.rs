use crate::config::Sshconfig;
use anyhow::Result;
use ssh2::Session;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn compress_and_deploy(
    version: &str,
    sess: &Session,
    is_history: bool,
    config: &Sshconfig,
) -> Result<()> {
    let history_path = Path::new(&config.history_path);
    let sftp = sess.sftp()?;

    // 如果不是历史版本，需要先构建和压缩
    if !is_history {
        // 1. 压缩 dist 目录
        println!("开始压缩 dist 目录...");
        let home = env::var("HOME")?;
        let project_dir = PathBuf::from(home).join("jobabc/jobabc-internal-admin");
        let dist_dir = project_dir.join("dist");

        if !dist_dir.exists() {
            anyhow::bail!("dist 目录不存在");
        }

        // 创建临时 zip 文件
        let temp_zip = project_dir.join("temp.zip");
        let zip_file = project_dir.join(format!("{}.zip", version));

        // 使用 zip 命令压缩
        let status = Command::new("zip")
            .arg("-r")
            .arg(&temp_zip)
            .arg("dist")
            .current_dir(&project_dir)
            .status()?;

        if !status.success() {
            anyhow::bail!("压缩失败");
        }

        // 重命名为版本号
        fs::rename(&temp_zip, &zip_file)?;

        // 读取文件内容
        let zip_content = fs::read(&zip_file)?;

        // 上传到历史版本目录
        println!("开始上传到历史版本目录...");
        let remote_path = history_path.join(version);
        let mut remote_file = sftp.create(&remote_path)?;
        remote_file.write_all(&zip_content)?;
        drop(remote_file);

        // 清理本地文件
        fs::remove_file(&zip_file)?;
        fs::remove_dir_all(&dist_dir)?;
    }

    // 2. 部署到生产目录
    println!("开始部署到生产目录...");
    let deploy_path = Path::new(&config.output_path);

    // 检查部署目录是否存在
    let mut channel = sess.channel_session()?;
    channel.exec(&format!("cd / && [ -d {} ]", deploy_path.display()))?;

    // 读取命令输出
    let mut output = String::new();
    channel.read_to_string(&mut output)?;

    let exit_status = channel.exit_status()?;
    channel.wait_close()?;

    // 如果目录不存在，创建它
    if exit_status != 0 {
        println!("创建部署目录...");
        let mut channel = sess.channel_session()?;
        channel.exec(&format!("mkdir -p {}", deploy_path.display()))?;

        // 读取命令输出
        let mut output = String::new();
        channel.read_to_string(&mut output)?;

        channel.wait_close()?;
    }

    // 从历史目录复制文件到部署目录
    println!("从历史版本复制文件...");
    let mut channel = sess.channel_session()?;
    channel.exec(&format!(
        "cp {}/{} {}/{}",
        history_path.display(),
        version,
        deploy_path.display(),
        version
    ))?;

    // 读取命令输出
    let mut output = String::new();
    channel.read_to_string(&mut output)?;
    channel.wait_close()?;

    // 解压文件
    let mut channel = sess.channel_session()?;
    channel.exec(&format!(
        "cd {} && unzip -o {} && rm {} && chmod -R 755 dist",
        deploy_path.display(),
        version,
        version
    ))?;

    // 读取命令输出
    let mut output = String::new();
    channel.read_to_string(&mut output)?;
    channel.wait_close()?;

    // 3. 重启 nginx
    println!("重启 nginx...");
    let mut channel = sess.channel_session()?;
    channel.exec("nginx -s reload")?;

    // 读取命令输出
    let mut output = String::new();
    channel.read_to_string(&mut output)?;
    channel.wait_close()?;

    println!("部署完成！");
    Ok(())
}
