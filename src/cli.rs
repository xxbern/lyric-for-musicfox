//! CLI 解析：clap derive 模式
//! - `--settings` 是 **长选项 flag**（不是 subcommand）
//! - `--benchmark` 是 P5 才用的开发选项，P0 仅声明不做实际行为

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "lyric-for-musicfox",
    version = "0.2.0",
    about = "lyric-for-musicfox 桌面歌词叠加应用",
    long_about = None,
)]
pub struct Cli {
    /// 启动设置界面（P1+ 才可用；P0 输出 stderr + exit 4）
    #[arg(long)]
    pub settings: bool,

    /// 开发期：启用 trace 日志输出（UDP→渲染延迟统计）
    #[arg(long)]
    pub benchmark: bool,
}

impl Cli {
    pub fn settings_requested(&self) -> bool {
        self.settings
    }
}
