use dialoguer::{Select, theme::ColorfulTheme};
use jobabc_internal_admin;

fn main() {
    let projects = vec!["jobabc-internal-admin", "退出"];

    loop {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择要运行的项目")
            .items(&projects)
            .default(0)
            .interact()
            .unwrap();

        match selection {
            0 => {
                println!("正在运行 jobabc-internal-admin...");
                let actions = vec!["打包 dev", "打包 prod", "返回主菜单"];

                let action = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("请选择操作")
                    .items(&actions)
                    .default(0)
                    .interact()
                    .unwrap();

                // 根据 action 选择打包环境
                match action {
                    0 => {
                        let env = String::from("dev");
                        println!("正在打包 dev 环境...");
                        match jobabc_internal_admin::config::read_config(&env) {
                            Ok(()) => println!("配置加载成功"),
                            Err(e) => println!("配置加载失败: {}", e),
                        }
                        break;
                    }
                    1 => {
                        let env = String::from("prod");
                        println!("正在打包 prod 环境... {}", env);
                        match jobabc_internal_admin::config::read_config(&env) {
                            Ok(()) => println!("配置加载成功"),
                            Err(e) => println!("配置加载失败: {}", e),
                        }
                        break;
                    }
                    2 => {
                        println!("返回主菜单");
                        continue;
                    }
                    _ => unreachable!(),
                }
            }
            1 => {
                println!("退出程序");
                break;
            }
            _ => unreachable!(),
        }
    }
}
