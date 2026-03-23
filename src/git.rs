use anyhow::{Context, Result};
use std::process::Command;

/// 获取指定远程的所有远程分支名称
/// 返回格式如：["origin/main", "origin/feature/login", ...]
pub fn list_remote_branches(remote_name: &str) -> Result<Vec<String>> {
    let mut branches = Vec::new();

    // 使用 git branch -r 获取远程分支列表
    let output = Command::new("git")
        .args(["branch", "-r", "--format", "%(refname:short)"])
        .output()
        .context("执行 git 命令失败")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            // 跳过 HEAD 引用
            if line.contains("->") {
                continue;
            }
            // 精确匹配：必须是 "origin/xxx" 格式，且 remote_name 后面要有 /
            let prefix = format!("{}/", remote_name);
            if line.starts_with(&prefix) && line.len() > prefix.len() {
                branches.push(line.to_string());
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git branch -r 失败：{}", stderr);
    }

    Ok(branches)
}

/// 后台 fetch 远程仓库（异步更新）
pub fn fetch_remote_async(remote_name: &str) {
    let remote_name = remote_name.to_string();
    std::thread::spawn(move || {
        let _ = Command::new("git")
            .args(["fetch", &remote_name, "--prune", "--quiet"])
            .output();
    });
}

/// 获取所有本地分支的短名称
pub fn list_local_branches() -> Result<Vec<String>> {
    let mut branches = Vec::new();

    let output = Command::new("git")
        .args(["branch", "--format", "%(refname:short)"])
        .output()
        .context("执行 git 命令失败")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim().trim_start_matches('*').trim();
            branches.push(line.to_string());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git branch 失败：{}", stderr);
    }

    Ok(branches)
}

/// 基于远程分支创建本地分支
/// remote_ref: 远程分支引用，如 "origin/feature/login"
/// branch_name: 新本地分支名称，如 "feature/login"
pub fn create_local_branch(remote_ref: &str, branch_name: &str) -> Result<()> {
    // 使用 git 命令创建，更可靠
    // 先检查远程引用是否存在
    let check_output = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &format!("refs/remotes/{}", remote_ref)])
        .output();

    if let Ok(output) = check_output {
        if !output.status.success() {
            anyhow::bail!("远程分支引用 '{}' 不存在，请先执行 'git fetch'", remote_ref);
        }
    }

    // 使用 git checkout -b 创建并跟踪远程分支
    let output = Command::new("git")
        .args(["checkout", "-b", branch_name, "--track", &remote_ref])
        .output()
        .context("执行 git checkout 命令失败")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("创建分支失败：{}", stderr.trim())
    }
}
