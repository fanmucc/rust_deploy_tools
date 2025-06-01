use crate::build;
use crate::history;
use crate::ssh;
use crate::version;
use prettytable::{Table, format, row};
use serde::Deserialize;
use std::env;
use std::io::{self, Write};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct Sshconfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub history_path: String,
    pub output_path: String,
    pub build: String,
    pub git_checkout: String,
}

pub type EnvConfig = HashMap<String, Sshconfig>;

pub fn read_config(env: &str) -> anyhow::Result<()> {
    // 加载 .env 文件
    let env_path = PathBuf::from("jobabc-internal-admin/.env");
    println!("尝试加载 .env 文件: {}", env_path.display());
    dotenv::from_path(&env_path).ok();

    println!("当前环境: {}", env);
    println!("环境变量 DEV_HOST: {:?}", env::var("DEV_HOST"));
    println!("环境变量 DEV_USERNAME: {:?}", env::var("DEV_USERNAME"));
    println!("环境变量 PROD_HOST: {:?}", env::var("PROD_HOST"));
    println!("环境变量 PROD_USERNAME: {:?}", env::var("PROD_USERNAME"));

    let content = fs::read_to_string("jobabc-internal-admin/config/config.json")?;
    let config: EnvConfig = serde_json::from_str(&content)?;
    let base_config = config.get(env).expect(&format!("没有找到 {} 配置", env));

    // 替换环境变量
    let host = env::var(format!("{}_HOST", env.to_uppercase())).map_err(|_| {
        anyhow::anyhow!(
            "未找到环境变量 {}_HOST，请检查 .env 文件",
            env.to_uppercase()
        )
    })?;
    let username = env::var(format!("{}_USERNAME", env.to_uppercase())).map_err(|_| {
        anyhow::anyhow!(
            "未找到环境变量 {}_USERNAME，请检查 .env 文件",
            env.to_uppercase()
        )
    })?;

    let env_config = Sshconfig {
        host,
        username,
        port: base_config.port,
        history_path: base_config.history_path.clone(),
        output_path: base_config.output_path.clone(),
        build: base_config.build.clone(),
        git_checkout: base_config.git_checkout.clone(),
    };

    // 读取 id_rsa
    let home = env::var("HOME")?;
    let id_rsa_path = if env == "dev" {
        PathBuf::from(home).join(".ssh").join("id_rsa")
    } else {
        PathBuf::from(home).join("jobabc").join("job123")
    };

    let id_rsa = fs::read_to_string(&id_rsa_path)?;

    println!("SSH连接配置: {:?}", env_config);
    let sess = ssh::connect(&env_config, &id_rsa)?;
    let history_files = history::get_history(&env_config, &sess)?;

    // 创建表格
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);

    // 添加表头
    table.add_row(row![
        "序号",
        "版本号",
        "部署时间",
        "序号",
        "版本号",
        "部署时间"
    ]);

    // 添加数据行，每行显示10个版本
    for i in 0..10 {
        let row1 = if i < history_files.len() {
            format!("{}", i + 1)
        } else {
            "".to_string()
        };
        let ver1 = if i < history_files.len() {
            history_files[i].clone()
        } else {
            "".to_string()
        };

        let row2 = if i + 10 < history_files.len() {
            format!("{}", i + 11)
        } else {
            "".to_string()
        };
        let ver2 = if i + 10 < history_files.len() {
            history_files[i + 10].clone()
        } else {
            "".to_string()
        };

        table.add_row(row![row1, ver1, "", row2, ver2, ""]);
    }

    println!("\n历史版本列表:");
    table.printstd();

    match version::show_version_menu()? {
        ref s if s == "版本自增" => {
            if let Some(latest) = version::get_latest_version(&history_files) {
                let new_version = latest.increment();
                println!("新版本号: {}", new_version.to_string());
                build::build_project(&new_version.to_string(), &sess, false, &env_config)?;
            } else {
                println!("没有找到历史版本，将创建 v1.0.0.zip");
                build::build_project("v1.0.0.zip", &sess, false, &env_config)?;
            }
        }
        ref s if s == "指定版本" => {
            print!("请输入版本号 (格式: v1.0.0): ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if version::validate_version(input) {
                println!("版本号有效: {}", input);
                build::build_project(&format!("{}.zip", input), &sess, false, &env_config)?;
            } else {
                println!("无效的版本号格式");
            }
        }
        ref s if s == "历史版本" => {
            if let Some(selected) = version::select_history_version(&history_files)? {
                println!("选择的版本: {}", selected);
                build::build_project(&selected, &sess, true, &env_config)?;
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
