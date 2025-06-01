use crate::config::Sshconfig;
use crate::ssh;
use anyhow::Result;
use ssh2::Session;

// 获取历史记录
pub fn get_history(config: &Sshconfig, sess: &Session) -> Result<Vec<String>> {
    let output = ssh::execute_command(sess, &format!("cd {} && ls -l", config.history_path))?;
    let history = output.split("\n").collect::<Vec<&str>>();
    let history_files = history
        .iter()
        .filter(|s| !s.is_empty()) // 过滤空行
        .map(|s| s.split_whitespace().nth(8).unwrap_or("").to_string())
        .filter(|s| !s.is_empty()) // 过滤空文件名
        .collect::<Vec<String>>();
    Ok(history_files)
}
