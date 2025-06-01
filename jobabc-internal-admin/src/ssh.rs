use crate::config::Sshconfig;
use anyhow::{Context, Result};
use ssh2::Session;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

pub fn connect(config: &Sshconfig, private_key: &str) -> Result<Session> {
    println!("正在连接到 {}:{}", config.host, config.port);

    // 启动动态加载动画
    let loading_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;

    // 在新线程中显示动画
    let handle = thread::spawn(move || {
        loop {
            print!("\r{} 连接中...", loading_chars[i]);
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(100));
            i = (i + 1) % loading_chars.len();
        }
    });

    // 尝试连接
    let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
        .with_context(|| format!("无法连接到服务器 {}:{}", config.host, config.port))?;
    let mut sess = Session::new().with_context(|| "创建SSH会话失败")?;
    sess.set_tcp_stream(tcp);
    sess.handshake().with_context(|| "SSH握手失败")?;

    // 停止动画
    handle.thread().unpark();
    print!("\r✓ 连接成功！\n");

    println!("正在使用密钥认证...");
    // 使用私钥认证
    sess.userauth_pubkey_memory(&config.username, None, private_key, None)
        .with_context(|| format!("SSH密钥认证失败，用户名: {}", config.username))?;

    if !sess.authenticated() {
        return Err(anyhow::anyhow!("SSH认证失败，请检查密钥是否正确"));
    }

    println!("SSH连接成功！");
    Ok(sess)
}

pub fn execute_command(sess: &Session, command: &str) -> Result<String> {
    let mut channel = sess.channel_session()?;
    channel.exec(command)?;

    let mut output = String::new();
    channel.read_to_string(&mut output)?;
    channel.wait_close()?;

    Ok(output)
}
