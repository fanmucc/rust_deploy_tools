#[derive(Debug, Clone)]
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
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
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

pub mod build {
    use super::Version;
    use anyhow::{Context, Result};
    use dialoguer::{Select, theme::ColorfulTheme};
    use prettytable::{Table, format, row};
    use ssh2::Session;
    use std::cmp::Ordering;
    use std::io::Read;
    use std::io::{self, Write};
    use std::net::TcpStream;
    use std::process::Command;
    use std::{collections::HashMap, env, fs, path::PathBuf};

    #[derive(Debug, serde::Deserialize)]
    pub struct Config {
        pub host: String,
        pub username: String,
        pub port: u16,
        pub history_path: String,
        pub output_path: String,
        pub build: String,
        pub git_checkout: String,
        pub local_path: String,
    }

    pub type EnvConfig = HashMap<String, Config>;

    // 1. 这里进入打包流程，接入传入参数为相应环境
    pub fn main(env: &str) -> anyhow::Result<()> {
        // 1. 获取配置 加载.env 文件
        let env_path = PathBuf::from("jobabc-internal-next/.env");
        dotenv::from_path(&env_path).ok();

        // 获取配置文件
        let content = fs::read_to_string("jobabc-internal-next/config/config.json")?;
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

        // 配置 env 参数
        let env_config = Config {
            host,
            username,
            port: base_config.port,
            history_path: base_config.history_path.clone(),
            output_path: base_config.output_path.clone(),
            build: base_config.build.clone(),
            git_checkout: base_config.git_checkout.clone(),
            local_path: base_config.local_path.clone(),
        };

        let home = env::var("HOME")?;
        let id_rsa_path = if env == "dev" {
            PathBuf::from(home).join(".ssh").join("id_rsa")
        } else {
            PathBuf::from(home).join("jobabc").join("job123")
        };
        println!("env_config: {:?}", env_config);
        let id_rsa = fs::read_to_string(&id_rsa_path)?;
        // 2. 链接 ssh
        let sess = ssh2(&env_config, &id_rsa)?;
        // 3. 输出历史版本 根据链接sess 获取历史版本
        let history_files = get_history(&sess, &env_config)?;

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
        // 4. 选择发布版本
        match show_version_menu()? {
            ref s if s == "版本自增" => {
                if let Some(latest) = get_latest_version(&history_files) {
                    let new_version = latest.increment();
                    println!("新版本号: {}", new_version.to_string());
                    build_project(&new_version.to_string(), &sess, false, &env_config, &env)?;
                } else {
                    println!("没有找到历史版本，将创建 v1.0.0.zip");
                    build_project("v1.0.0.zip", &sess, false, &env_config, &env)?;
                }
            }
            ref s if s == "指定版本" => {
                print!("请输入版本号 (格式: v1.0.0): ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let input = input.trim();

                if validate_version(input) {
                    println!("版本号有效: {}", input);
                    // build::build_project(&format!("{}.zip", input), &sess, false, &env_config)?;
                } else {
                    println!("无效的版本号格式");
                }
            }
            ref s if s == "历史版本" => {
                if let Some(selected) = select_history_version(&history_files)? {
                    println!("选择的版本: {}", selected);
                    // build::build_project(&selected, &sess, true, &env_config)?;
                }
            }
            _ => unreachable!(),
        }
        // 5. 对相应项目进行打包
        // 6. 压缩
        // 7. 上传
        // 8. 解压并移除
        // 9. 项目进行部署
        // 10. 完成
        Ok(())
    }

    fn ssh2(config: &Config, private_key: &str) -> Result<Session> {
        // 尝试链接
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .with_context(|| format!("无法连接到服务器 {}:{}", config.host, config.port))?;
        let mut sess = Session::new().with_context(|| "创建SSH会话失败")?;
        // 创建链接
        sess.set_tcp_stream(tcp);
        sess.handshake().with_context(|| "SSH握手失败")?;

        println!("正在使用密钥认证...");
        sess.userauth_pubkey_memory(&config.username, None, private_key, None)
            .with_context(|| format!("SSH密钥认证失败，用户名: {}", config.username))?;

        if !sess.authenticated() {
            return Err(anyhow::anyhow!("SSH认证失败，请检查密钥是否正确"));
        }

        println!("SSH连接成功！");

        Ok(sess)
    }

    fn get_history(sess: &Session, env_config: &Config) -> Result<Vec<String>> {
        let output = execute_command(sess, &format!("cd {} && ls -l", env_config.history_path))?;
        let history = output.split("\n").collect::<Vec<&str>>();
        let history_files = history
            .iter()
            .filter(|s| !s.is_empty()) // 过滤空行
            .map(|s| s.split_whitespace().nth(8).unwrap_or("").to_string())
            .filter(|s| !s.is_empty()) // 过滤空文件名
            .collect::<Vec<String>>();
        Ok(history_files)
    }

    fn execute_command(sess: &Session, command: &str) -> Result<String> {
        let mut channel = sess.channel_session()?;
        channel.exec(command)?;

        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        Ok(output)
    }

    fn show_version_menu() -> Result<String> {
        let options = vec!["版本自增", "指定版本", "历史版本"];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择版本管理方式")
            .items(&options)
            .default(0)
            .interact()?;

        Ok(options[selection].to_string())
    }

    fn get_latest_version(history: &[String]) -> Option<Version> {
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

    // 打包
    fn build_project(
        version: &str,
        sess: &Session,
        is_history: bool,
        config: &Config,
        env_type: &str,
    ) -> Result<()> {
        if !is_history {
            println!("开始构建项目...");

            // 进入nest 项目
            let home = env::var("HOME")?;
            let project_dir = PathBuf::from(home).join(&config.local_path);
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
            if !status.status.success() {
                anyhow::bail!("git 状态检查失败");
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
            zip_project(version, sess, is_history, config, &env_type)?;
        }

        Ok(())
    }

    fn zip_project(
        version: &str,
        sess: &Session,
        is_history: bool,
        config: &Config,
        env_type: &str,
    ) -> Result<()> {
        let history_path = PathBuf::from(&config.history_path);
        let sftp = sess.sftp()?;

        // 如果不是历史版本，需要先构建和压缩
        if !is_history {
            println!("开始压缩项目...");
            let home = env::var("HOME")?;
            let project_dir = PathBuf::from(home).join(&config.local_path);
            let dist_dir = project_dir.join("dist");
            let prisma_dir = project_dir.join("prisma");

            if !dist_dir.exists() {
                anyhow::bail!("dist 目录不存在");
            }
            if !prisma_dir.exists() {
                anyhow::bail!("prisma 目录不存在");
            }

            // 创建临时 zip 文件
            let temp_zip = project_dir.join("temp.zip");
            let zip_file = project_dir.join(format!("{}.zip", version));

            // 使用 zip 命令压缩 dist 和 prisma 目录
            let status = Command::new("zip")
                .arg("-r")
                .arg(&temp_zip)
                .arg("dist")
                .arg("prisma")
                .current_dir(&project_dir)
                .status()?;

            if !status.success() {
                anyhow::bail!("压缩失败");
            }

            // 重命名临时文件
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

        // 部署到生产目录
        println!("开始部署到生产目录...");
        let deploy_path = PathBuf::from(&config.output_path);

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
        println!("解压文件...");
        let mut channel = sess.channel_session()?;
        channel.exec(&format!(
            "cd {} && unzip -o {} && rm {} && chmod -R 755 dist prisma",
            deploy_path.display(),
            version,
            version
        ))?;

        // 读取命令输出
        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        if env_type == String::from("dev") {
            // 拉去 prisma
            println!("拉取 prisma 文件");
            let mut channel = sess.channel_session()?;
            channel.exec("cd /home/www/jobabc-internal && npx prisma db pull")?;

            println!("同步 prisma 文件");
            let mut channel = sess.channel_session()?;
            channel.exec("cd /home/www/jobabc-internal && npx prisma generate")?;

            // 重启 pm2
            println!("重启 pm2");
            let mut channel = sess.channel_session()?;
            channel.exec("pm2 restart jobabc-internal-dev")?;
        } else {
            // 生产模式进行进行迁徙文件同步
            println!("生产模式进行进行迁徙文件同步");
            let mut channel = sess.channel_session()?;
            channel.exec("cd /home/www/jobabc-internal && npx prisma migrate deploy")?;

            println!("同步 prisma 文件");
            let mut channel = sess.channel_session()?;
            channel.exec("cd /home/www/jobabc-internal && npx prisma generate")?;

            // 重启 pm2
            println!("重启 pm2");
            let mut channel = sess.channel_session()?;
            channel.exec("pm2 restart jobabc-internal")?;
        }

        println!("部署完成！");

        Ok(())
    }
}
