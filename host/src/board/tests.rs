use super::*;
use crate::remote::identity::Identity;
use crate::store::BotConfig;
use std::path::PathBuf;
use std::sync::Arc;

fn hub() -> Arc<Hub> {
    let dir = std::env::temp_dir().join(format!("codync-board-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(Path::new(":memory:")).unwrap();
    let hub = Hub::new(store, "board-test".into(), Identity::load_or_create(&dir).unwrap(), "test".into(), 0);
    for id in ["ada", "bo"] {
        let cfg: BotConfig =
            serde_json::from_value(json!({"id": id, "name": id.to_uppercase(), "backend": "claude", "cwd": "/tmp"}))
                .unwrap();
        hub.store.save_bot(&cfg).unwrap();
    }
    hub
}

fn new_task(hub: &Hub, title: &str, project: Option<&str>) -> Task {
    create(hub, Actor::User, &NewTask { title, detail: "", project }).unwrap()
}

/// git in `dir` alone: a pre-commit hook running these tests sets `GIT_DIR` for the commit.
fn git(dir: &Path, args: &[&str]) -> String {
    let mut cmd = std::process::Command::new("git");
    for (key, _) in std::env::vars_os().filter(|(k, _)| k.to_string_lossy().starts_with("GIT_")) {
        cmd.env_remove(key);
    }
    let out = cmd.args(["-c", "user.email=t@t", "-c", "user.name=t"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codync-board-repo-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

#[tokio::test]
async fn the_first_claimer_wins_and_a_bot_holds_at_most_three() {
    let hub = hub();
    let task = new_task(&hub, "Write the parser", None);
    assert_eq!(task.status, TaskStatus::Open);
    assert_eq!(task.created_by, None);

    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    assert_eq!((claimed.status, claimed.claimed_by.as_deref()), (TaskStatus::Claimed, Some("ada")));
    assert!(claim(&hub, &task.id, "ada").await.is_ok(), "claiming again is a no-op for the claimer");
    let taken = claim(&hub, &task.id, "bo").await.unwrap_err().to_string();
    assert_eq!(taken, "ADA already claimed this task");

    for title in ["two", "three"] {
        let t = new_task(&hub, title, None);
        claim(&hub, &t.id, "ada").await.unwrap();
    }
    let fourth = new_task(&hub, "four", None);
    assert!(claim(&hub, &fourth.id, "ada").await.unwrap_err().to_string().contains("already hold 3 tasks"));
    assert!(claim(&hub, &fourth.id, "bo").await.is_ok());
}

#[tokio::test]
async fn release_note_and_finish_without_a_repository() {
    let hub = hub();
    let task = new_task(&hub, "Look into the crash", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    assert!(release(&hub, &task.id, Actor::Bot("bo"), None).unwrap_err().to_string().contains("holds this task"));

    let released = release(&hub, &task.id, Actor::Bot("ada"), Some("needs a Mac")).unwrap();
    assert_eq!((released.status, released.claimed_by.clone()), (TaskStatus::Open, None));
    assert_eq!(released.notes[0].text, "Released: needs a Mac");

    claim(&hub, &task.id, "bo").await.unwrap();
    for i in 0..(MAX_NOTES + 5) {
        note(&hub, &task.id, Actor::Bot("bo"), &format!("finding {i}")).unwrap();
    }
    let noted = get(&hub.store, &task.id).unwrap();
    assert_eq!(noted.notes.len(), MAX_NOTES);
    assert_eq!(noted.notes.last().unwrap().text, format!("finding {}", MAX_NOTES + 4), "the oldest go first");

    assert!(finish(&hub, &task.id, "bo", " ").await.unwrap_err().to_string().contains("summary"));
    let done = finish(&hub, &task.id, "bo", "Fixed the null check; ran the tests").await.unwrap();
    assert_eq!(done.status, TaskStatus::Done, "no branch: nothing to review");
    assert!(claim(&hub, &task.id, "ada").await.unwrap_err().to_string().contains("only an open task"));
}

#[tokio::test]
async fn a_git_task_works_on_its_own_worktree_and_merges() {
    let hub = hub();
    let project = repo();
    let task = new_task(&hub, "Add b", Some(&project.display().to_string()));

    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    let tree = PathBuf::from(claimed.worktree.clone().expect("a worktree"));
    let branch = claimed.branch.clone().expect("a branch");
    assert_eq!(branch, git::branch(&task.id));
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), branch);

    std::fs::write(tree.join("b.txt"), "two\n").unwrap();
    let dirty = finish(&hub, &task.id, "ada", "Added b").await.unwrap_err().to_string();
    assert!(dirty.contains("uncommitted"), "{dirty}");
    git(&tree, &["add", "b.txt"]);
    git(&tree, &["commit", "-q", "-m", "Add b"]);
    assert!(!project.join("b.txt").exists(), "the project's own checkout is untouched");

    let review = finish(&hub, &task.id, "ada", "Added b").await.unwrap();
    assert_eq!(review.status, TaskStatus::Review);

    let merged = merge(&hub, &task.id).await.unwrap();
    assert_eq!((merged.status, merged.worktree.clone(), merged.branch.clone()), (TaskStatus::Done, None, None));
    assert_eq!(std::fs::read_to_string(project.join("b.txt")).unwrap(), "two\n");
    assert!(!tree.exists(), "the worktree goes");
    assert!(git(&project, &["branch", "--list", &branch]).is_empty(), "the merged branch goes");
    std::fs::remove_dir_all(&project).unwrap();
}

#[tokio::test]
async fn a_conflicting_merge_leaves_the_project_as_it_was() {
    let hub = hub();
    let project = repo();
    let task = new_task(&hub, "Change a", Some(&project.display().to_string()));
    let tree = PathBuf::from(claim(&hub, &task.id, "ada").await.unwrap().worktree.unwrap());
    std::fs::write(tree.join("a.txt"), "theirs\n").unwrap();
    git(&tree, &["commit", "-q", "-am", "theirs"]);
    finish(&hub, &task.id, "ada", "Changed a").await.unwrap();
    std::fs::write(project.join("a.txt"), "ours\n").unwrap();
    git(&project, &["commit", "-q", "-am", "ours"]);

    let error = merge(&hub, &task.id).await.unwrap_err().to_string();
    assert!(error.contains("doesn't merge cleanly"), "{error}");
    assert_eq!(std::fs::read_to_string(project.join("a.txt")).unwrap(), "ours\n");
    assert!(git(&project, &["status", "--porcelain"]).is_empty(), "the merge was aborted");
    assert_eq!(get(&hub.store, &task.id).unwrap().status, TaskStatus::Review);

    let cancelled = set_status(&hub, &task.id, TaskStatus::Cancelled).await.unwrap();
    assert_eq!(cancelled.worktree, None);
    assert!(!git(&project, &["branch", "--list", &git::branch(&task.id)]).is_empty(), "the branch stays");
    std::fs::remove_dir_all(&project).unwrap();
}

#[tokio::test]
async fn changes_sync_as_task_events_and_deletes_leave_a_tombstone() {
    let hub = hub();
    let mut events = hub.events.subscribe();
    let task = new_task(&hub, "Ship it", None);
    let event = events.try_recv().unwrap();
    assert_eq!((event["type"].as_str(), event["task"]["id"].as_str()), (Some("task"), Some(task.id.as_str())));

    claim(&hub, &task.id, "ada").await.unwrap();
    assert!(delete(&hub, &task.id).await.unwrap_err().to_string().contains("claimed"));
    set_status(&hub, &task.id, TaskStatus::Open).await.unwrap();
    delete(&hub, &task.id).await.unwrap();
    assert!(get(&hub.store, &task.id).is_err());
    assert_eq!(list(&hub.store).len(), 0);
    let caught_up = since(&hub.store, task.rev);
    assert_eq!(caught_up.len(), 1);
    assert_eq!(caught_up[0]["task"]["deleted"], true, "a client that had it drops it");
}

#[test]
fn titles_and_projects_are_checked() {
    assert!(title("").is_err());
    assert!(title("two\nlines").is_err());
    assert!(title(&"x".repeat(MAX_TITLE + 1)).is_err());
    assert_eq!(title("  Fix it  ").unwrap(), "Fix it");
    assert!(project(Some("relative/path")).is_err());
    assert!(project(Some("/definitely/not/here")).is_err());
    assert_eq!(project(Some("  ")).unwrap(), None);
}

#[tokio::test]
async fn bots_use_the_board_through_their_team_tools() {
    let hub = hub();
    let made = tools::call(&hub, "ada", "create_task", &json!({"title": "Port the CLI", "project": ""})).await.unwrap();
    let id = made["task"]["id"].as_str().unwrap().to_owned();
    assert_eq!(made["task"]["createdBy"], "ADA");
    assert_eq!(made["task"]["project"], "/tmp", "the bot's own folder by default");

    tools::call(&hub, "bo", "claim_task", &json!({"taskId": id})).await.unwrap();
    let mine = tools::call(&hub, "bo", "list_tasks", &json!({"mine": true})).await.unwrap();
    assert_eq!(mine["tasks"][0]["claimedBy"], "BO");
    let theirs = tools::call(&hub, "ada", "list_tasks", &json!({"mine": true})).await.unwrap();
    assert_eq!(theirs["tasks"].as_array().unwrap().len(), 0);
    let open = tools::call(&hub, "ada", "list_tasks", &json!({"status": "open"})).await.unwrap();
    assert_eq!(open["tasks"].as_array().unwrap().len(), 0);
    assert!(tools::call(&hub, "ada", "finish_task", &json!({"taskId": id, "summary": "x"})).await.is_err());
}
