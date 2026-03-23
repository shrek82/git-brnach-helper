use anyhow::{Context, Result};
use git2::Repository;
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
            // 只包含指定远程的分支
            if line.starts_with(remote_name) {
                branches.push(line.to_string());
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git branch -r 失败：{}", stderr);
    }

    Ok(branches)
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
    let repo = Repository::discover(".")
        .context("当前目录不是 git 仓库")?;

    // 查找远程引用
    let remote_oid = repo
        .find_reference(remote_ref)
        .context(format!("找不到远程分支引用：{}", remote_ref))?
        .target()
        .context("无法获取远程分支的 commit")?;

    // 创建本地分支
    let commit = repo.find_commit(remote_oid)?;

    // 检查分支是否已存在
    if repo.find_branch(branch_name, git2::BranchType::Local).is_ok() {
        anyhow::bail!("本地分支 '{}' 已存在", branch_name);
    }

    // 创建分支
    repo.branch(branch_name, &commit, false)?;

    Ok(())
}
