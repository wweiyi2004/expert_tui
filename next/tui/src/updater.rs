use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(install: bool) -> Result<Value> {
    let script = std::env::var_os("EXPERT_UPDATE_SCRIPT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()?
                .parent()
                .map(|p| p.join("lib/update.cjs"))
        })
        .context("请通过 npm 安装的 expert 启动以使用更新功能")?;
    anyhow::ensure!(
        script.is_absolute() && script.is_file(),
        "当前安装缺少更新器，请使用独立 npm 发行包"
    );
    let node = std::env::var_os("EXPERT_NODE_EXE").unwrap_or_else(|| "node".into());
    let mut command = Command::new(node);
    command
        .arg(&script)
        .arg(if install { "install" } else { "check" })
        .arg("--json")
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .context("无法启动 Node.js 更新器；请安装 Node.js 20+")?;
    anyhow::ensure!(output.stdout.len() < 1024 * 1024, "更新器响应超过限制");
    let value: Value =
        serde_json::from_slice(&output.stdout).context("更新器未返回有效状态；当前安装未切换")?;
    if !output.status.success() || value["ok"] != true {
        let message: String = value["message"]
            .as_str()
            .unwrap_or("更新失败，当前版本保留")
            .chars()
            .filter(|c| !c.is_control())
            .take(2048)
            .collect();
        bail!("{message}");
    }
    Ok(value)
}
