//! The git side of the board: a worktree and branch per claimed task, so bots working in one repository side by side
//! never edit the same checkout, and the merge the user starts from a task waiting for review.

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use std::path::{Path, PathBuf};

/// A task's own checkout.
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
}

/// The branch a task's work goes on.
pub fn branch(task: &str) -> String {
    format!("codync/task-{}", &task[..task.len().min(8)])
}

/// Where a task's worktree goes: `<data dir>/worktrees/<task id>`.
fn worktree_path(task: &str) -> PathBuf {
    crate::service::data_dir().join("worktrees").join(task)
}

/// Runs git in `dir` and answers its trimmed stdout; a failure carries git's stderr. `GIT_*` variables are left out,
/// so a hook's `GIT_DIR` can't send the command to another repository.
async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let program = crate::chat::calls::project::git_program().await.ok_or_else(|| anyhow!("git isn't installed"))?;
    let mut cmd = tokio::process::Command::new(program);
    for (key, _) in std::env::vars_os().filter(|(k, _)| k.to_string_lossy().starts_with("GIT_")) {
        cmd.env_remove(key);
    }
    let out = cmd.args(args).current_dir(dir).kill_on_drop(true).output().await.context("running git")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("git {}: {}", args.first().copied().unwrap_or_default(), err.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Whether `dir` is the top of a git repository's working tree (a subfolder or no git: the bot works there directly).
async fn is_repo_root(dir: &Path) -> bool {
    let Ok(top) = git(dir, &["rev-parse", "--show-toplevel"]).await else { return false };
    let canonical = |p: &Path| std::fs::canonicalize(p).ok();
    canonical(Path::new(&top)).is_some_and(|top| canonical(dir).as_ref() == Some(&top))
}

/// Makes the task's worktree in `project` (on its branch, new from `HEAD` or the one an earlier claim left), or
/// `None` when `project` isn't a git repository with a commit.
pub async fn checkout(project: &Path, task: &str) -> Result<Option<Worktree>> {
    if !is_repo_root(project).await || git(project, &["rev-parse", "--verify", "HEAD"]).await.is_err() {
        return Ok(None);
    }
    let (path, branch) = (worktree_path(task), branch(task));
    if path.join(".git").exists() {
        return Ok(Some(Worktree { path, branch }));
    }
    let parent = path.parent().context("worktrees folder")?.to_owned();
    tokio::task::spawn_blocking(move || std::fs::create_dir_all(parent)).await??;
    let target = path.display().to_string();
    let exists = git(project, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).await.is_ok();
    // A worktree folder deleted by hand is still registered; prune lets its path be used again.
    git(project, &["worktree", "prune"]).await?;
    if exists {
        git(project, &["worktree", "add", &target, &branch]).await?;
    } else {
        git(project, &["worktree", "add", "-b", &branch, &target, "HEAD"]).await?;
    }
    Ok(Some(Worktree { path, branch }))
}

/// Whether a worktree has no uncommitted changes to tracked or new files.
pub async fn clean(tree: &Path) -> Result<bool> {
    Ok(git(tree, &["status", "--porcelain"]).await?.is_empty())
}

/// Removes a task's worktree (its branch stays). A worktree already gone is fine.
pub async fn remove_worktree(project: &Path, tree: &Path) -> Result<()> {
    if !tree.exists() {
        let _ = git(project, &["worktree", "prune"]).await;
        return Ok(());
    }
    git(project, &["worktree", "remove", "--force", &tree.display().to_string()]).await.map(drop)
}

/// Merges `branch` into the project's checked-out branch with a merge commit. Refused while the project has
/// uncommitted changes to tracked files; a conflict is aborted, leaving the project as it was.
pub async fn merge(project: &Path, branch: &str, title: &str) -> Result<()> {
    ensure!(
        git(project, &["status", "--porcelain", "--untracked-files=no"]).await?.is_empty(),
        "{} has uncommitted changes: commit or stash them before merging",
        project.display()
    );
    let message = format!("Merge task: {title}");
    if let Err(error) = git(project, &["merge", "--no-ff", "--no-edit", "-m", &message, branch]).await {
        let _ = git(project, &["merge", "--abort"]).await;
        return Err(error.context(format!(
            "{branch} doesn't merge cleanly: reopen the task and ask its bot to merge the current branch into it"
        )));
    }
    Ok(())
}

/// Deletes a merged task branch (`-d`: git refuses an unmerged one).
pub async fn delete_branch(project: &Path, branch: &str) -> Result<()> {
    git(project, &["branch", "-d", branch]).await.map(drop)
}
