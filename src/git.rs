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
            // 处理两种格式：
            // 1. "origin/xxx" - 标准格式
            // 2. "remotes/origin/xxx" - 完整格式
            let branch_ref = if line.starts_with("remotes/") {
                // 移除 "remotes/" 前缀
                line.strip_prefix("remotes/").unwrap_or(line)
            } else {
                line
            };

            // 精确匹配：必须是 "origin/xxx" 格式，且 remote_name 后面要有 /
            let prefix = format!("{}/", remote_name);
            if branch_ref.starts_with(&prefix) && branch_ref.len() > prefix.len() {
                branches.push(branch_ref.to_string());
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git branch -r 失败：{}", stderr);
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

/// 同步已存在的本地分支到远程（git pull）
/// branch_name: 本地分支名称，如 "feature/login"
pub fn sync_local_branch(branch_name: &str) -> Result<()> {
    // 先切换到目标分支
    let checkout_output = Command::new("git")
        .args(["checkout", branch_name])
        .output()
        .context("执行 git checkout 命令失败")?;

    if !checkout_output.status.success() {
        let stderr = String::from_utf8_lossy(&checkout_output.stderr);
        anyhow::bail!("切换到分支 '{}' 失败：{}", branch_name, stderr.trim());
    }

    // 执行 git pull 同步
    let pull_output = Command::new("git")
        .args(["pull"])
        .output()
        .context("执行 git pull 命令失败")?;

    if pull_output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&pull_output.stderr);
        anyhow::bail!("同步分支 '{}' 失败：{}", branch_name, stderr.trim())
    }
}

/// 删除本地分支
/// branch_name: 要删除的分支名称，如 "feature/login"
/// force: 是否强制删除（即使未合并）
pub fn delete_local_branch(branch_name: &str, force: bool) -> Result<()> {
    // 获取当前分支
    let current_branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("执行 git rev-parse 命令失败")?;

    if !current_branch_output.status.success() {
        let stderr = String::from_utf8_lossy(&current_branch_output.stderr);
        anyhow::bail!("获取当前分支失败：{}", stderr.trim());
    }

    let current_branch = String::from_utf8_lossy(&current_branch_output.stdout).trim().to_string();

    // 不能删除当前分支
    if current_branch == branch_name {
        anyhow::bail!("不能删除当前所在的分支 '{}'", branch_name);
    }

    // 执行删除
    let delete_flag = if force { "-D" } else { "-d" };
    let output = Command::new("git")
        .args(["branch", delete_flag, branch_name])
        .output()
        .context("执行 git branch 命令失败")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("删除分支 '{}' 失败：{}", branch_name, stderr.trim())
    }
}

/// 切换到指定分支
/// branch_name: 要切换到的分支名称，如 "feature/login"
pub fn checkout_branch(branch_name: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["checkout", branch_name])
        .output()
        .context("执行 git checkout 命令失败")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("切换到分支 '{}' 失败：{}", branch_name, stderr.trim())
    }
}

/// 获取分支与远程的分歧状态（ahead/behind）
/// branch_name: 本地分支名称，如 "feature/login"
/// 返回 (ahead_count, behind_count) 表示领先和落后的提交数
pub fn get_branch_ahead_behind(branch_name: &str) -> Result<(usize, usize)> {
    // 获取对应的远程分支引用
    let remote_ref = format!("origin/{}", branch_name);

    // 使用 git rev-list 计算分歧
    let ahead_output = Command::new("git")
        .args(["rev-list", "--count", &format!("{}..{}", remote_ref, branch_name)])
        .output()
        .context("执行 git rev-list 命令失败")?;

    let behind_output = Command::new("git")
        .args(["rev-list", "--count", &format!("{}..{}", branch_name, remote_ref)])
        .output()
        .context("执行 git rev-list 命令失败")?;

    if ahead_output.status.success() && behind_output.status.success() {
        let ahead = String::from_utf8_lossy(&ahead_output.stdout).trim().parse().unwrap_or(0);
        let behind = String::from_utf8_lossy(&behind_output.stdout).trim().parse().unwrap_or(0);
        Ok((ahead, behind))
    } else {
        // 如果远程分支不存在，返回 (0, 0)
        Ok((0, 0))
    }
}

/// 获取分支的最近提交记录
/// branch_name: 分支名称，如 "feature/login"
/// 返回提交记录列表（最多 5 条）
pub fn get_recent_commits(branch_name: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "log",
            branch_name,
            "--format=%h %ad %s",
            "--date=short",
            "-n",
            "5",
        ])
        .output()
        .context("执行 git log 命令失败")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
        Ok(commits)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("获取提交记录失败：{}", stderr.trim())
    }
}

/// 获取分支的最后提交信息（时间、作者、消息）
/// branch_name: 分支名称，如 "feature/login"
pub fn get_last_commit_info(branch_name: &str) -> Result<(String, String, String)> {
    let output = Command::new("git")
        .args([
            "log",
            branch_name,
            "--format=%ar||%an||%s",
            "-n",
            "1",
        ])
        .output()
        .context("执行 git log 命令失败")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parts: Vec<&str> = stdout.split("||").collect();
        if parts.len() >= 3 {
            Ok((
                parts[0].to_string(),  // 相对时间，如 "2 days ago"
                parts[1].to_string(),  // 作者名
                parts[2].to_string(),  // 提交消息
            ))
        } else {
            Ok((String::from("未知"), String::from("未知"), String::from("未知")))
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("获取提交信息失败：{}", stderr.trim())
    }
}

/// 获取远程分支的最后提交信息（时间、作者、消息）
/// remote_ref: 远程分支引用，如 "origin/feature/login"
pub fn get_remote_last_commit_info(remote_ref: &str) -> Result<(String, String, String)> {
    let output = Command::new("git")
        .args([
            "log",
            remote_ref,
            "--format=%ar||%an||%s",
            "-n",
            "1",
        ])
        .output()
        .context("执行 git log 命令失败")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parts: Vec<&str> = stdout.split("||").collect();
        if parts.len() >= 3 {
            Ok((
                parts[0].to_string(),  // 相对时间，如 "2 days ago"
                parts[1].to_string(),  // 作者名
                parts[2].to_string(),  // 提交消息
            ))
        } else {
            Ok((String::from("未知"), String::from("未知"), String::from("未知")))
        }
    } else {
        // 远程分支不存在或已删除，返回默认值
        Ok((String::from("-"), String::from("-"), String::from("-")))
    }
}

/// 获取当前所在的分支名称
/// 返回当前分支的短名称，如 "feature/login"
pub fn get_current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("执行 git rev-parse 命令失败")?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("获取当前分支失败：{}", stderr.trim())
    }
}
