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
            serde_json::from_value(json!({"id": id, "name": id.to_uppercase(), "backend": "claude", "cwd": folder()}))
                .unwrap();
        hub.store.save_bot(&cfg).unwrap();
    }
    hub
}

/// The bots' folder: an absolute directory on every platform.
fn folder() -> String {
    std::env::temp_dir().display().to_string()
}

fn new_task(hub: &Hub, title: &str, project: Option<&str>) -> Task {
    create(hub, Actor::User, &NewTask { title, detail: "", project, parent: None }).unwrap()
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
    // The host's merge commit needs an identity, which a CI runner has none of.
    git(&dir, &["config", "user.email", "t@t"]);
    git(&dir, &["config", "user.name", "t"]);
    // Files read back as written: Windows' git would check them out with CRLF.
    git(&dir, &["config", "core.autocrlf", "false"]);
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
    let checking = finish(&hub, &task.id, "bo", "Fixed the null check; ran the tests").await.unwrap();
    assert_eq!(
        (checking.status, checking.reviewer.as_deref()),
        (TaskStatus::Checking, Some("ada")),
        "the other bot checks"
    );
    let done = flow::review(&hub, &task.id, "ada", true, "").unwrap();
    assert_eq!(done.status, TaskStatus::Done, "no branch: nothing to merge");
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

    assert_eq!(finish(&hub, &task.id, "ada", "Added b").await.unwrap().status, TaskStatus::Checking);
    let review = flow::review(&hub, &task.id, "bo", true, "b.txt holds two").unwrap();
    assert_eq!(review.status, TaskStatus::Review, "checked: waits for the user's merge");
    assert_eq!(review.notes.last().unwrap().text, "Checked: b.txt holds two");

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
    flow::review(&hub, &task.id, "bo", true, "").unwrap();
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
    assert_eq!(made["task"]["project"], folder(), "the bot's own folder by default");

    tools::call(&hub, "bo", "claim_task", &json!({"taskId": id})).await.unwrap();
    let mine = tools::call(&hub, "bo", "list_tasks", &json!({"mine": true})).await.unwrap();
    assert_eq!(mine["tasks"][0]["claimedBy"], "BO");
    let theirs = tools::call(&hub, "ada", "list_tasks", &json!({"mine": true})).await.unwrap();
    assert_eq!(theirs["tasks"].as_array().unwrap().len(), 0);
    let open = tools::call(&hub, "ada", "list_tasks", &json!({"status": "open"})).await.unwrap();
    assert_eq!(open["tasks"].as_array().unwrap().len(), 0);
    assert!(tools::call(&hub, "ada", "finish_task", &json!({"taskId": id, "summary": "x"})).await.is_err());
}

#[tokio::test]
async fn idle_bots_hear_about_open_tasks_once_unless_turned_off() {
    let hub = hub();
    dispatch::dispatch(&hub).unwrap();
    assert!(hub.board.nudges.told.locked().is_empty(), "no open task: nobody hears");

    dispatch::set_claims(&hub, "bo", false).unwrap();
    let first = new_task(&hub, "Write the docs", None);
    dispatch::dispatch(&hub).unwrap();
    let heard = |hub: &Hub| hub.board.nudges.told.locked().keys().cloned().collect::<Vec<_>>();
    assert_eq!(heard(&hub), ["ada"], "bo is turned off");
    let at = hub.board.nudges.told.locked()["ada"].1;
    dispatch::dispatch(&hub).unwrap();
    assert_eq!(hub.board.nudges.told.locked()["ada"].1, at, "the same open tasks: not told again");

    new_task(&hub, "Fix the build", None);
    dispatch::dispatch(&hub).unwrap();
    assert_eq!(hub.board.nudges.told.locked()["ada"].0.len(), 2, "new open tasks: told again");

    claim(&hub, &first.id, "ada").await.unwrap();
    assert_eq!(dispatch::opted_out(&hub.store).into_iter().collect::<Vec<_>>(), ["bo"]);
    dispatch::set_claims(&hub, "bo", true).unwrap();
    assert!(dispatch::opted_out(&hub.store).is_empty());
    assert!(dispatch::set_claims(&hub, "nobody", false).is_err());
}

#[tokio::test]
async fn a_goal_becomes_a_task_the_lead_plans() {
    let hub = hub();
    assert!(dispatch::split_goal(&hub, "  ").is_err(), "an empty goal");
    let lead: BotConfig =
        serde_json::from_value(json!({"id": "cy", "name": "CY", "backend": "codex", "cwd": folder(), "lead": true}))
            .unwrap();
    hub.store.save_bot(&lead).unwrap();
    let got = dispatch::split_goal(&hub, "Prepare the exam papers\nfor both kids").unwrap();
    assert_eq!(got["botId"], "cy", "the lead");
    let task = list(&hub.store).into_iter().next().unwrap();
    assert_eq!((task.title.as_str(), task.claimed_by.as_deref()), ("Prepare the exam papers", Some("cy")));
    assert_eq!(task.detail, "Prepare the exam papers\nfor both kids");
    assert!(task.worktree.is_none(), "planning claims no worktree");
}

#[tokio::test]
async fn a_coordinator_finishes_only_after_its_subtasks() {
    let hub = hub();
    let parent = new_task(&hub, "Run the workshop", None);
    flow::plan(&hub, &parent.id, false).unwrap();
    let coordinator = get(&hub.store, &parent.id).unwrap().claimed_by.unwrap();
    let worker = if coordinator == "ada" { "bo" } else { "ada" };

    let made = tools::call(&hub, &coordinator, "create_task", &json!({"title": "Slides", "parentId": parent.id}))
        .await
        .unwrap();
    let part = made["task"]["id"].as_str().unwrap().to_owned();
    assert_eq!(made["task"]["parentId"], parent.id.as_str());
    let read = tools::call(&hub, &coordinator, "read_task", &json!({"taskId": parent.id})).await.unwrap();
    assert_eq!(read["task"]["subtasks"][0]["title"], "Slides");

    let early = finish(&hub, &parent.id, &coordinator, "Assembled").await.unwrap_err().to_string();
    assert!(early.contains("1 subtasks"), "{early}");
    claim(&hub, &part, worker).await.unwrap();
    finish(&hub, &part, worker, "31 slides").await.unwrap();
    assert!(finish(&hub, &parent.id, &coordinator, "Assembled").await.is_err(), "still being checked");
    tools::call(&hub, &coordinator, "review_task", &json!({"taskId": part, "approve": true})).await.unwrap();
    let assembled = finish(&hub, &parent.id, &coordinator, "Assembled").await.unwrap();
    assert_eq!((assembled.status, assembled.reviewer.as_deref()), (TaskStatus::Checking, Some(worker)));

    let closed = new_task(&hub, "Old", None);
    set_status(&hub, &closed.id, TaskStatus::Done).await.unwrap();
    let orphan = NewTask { title: "Late", detail: "", project: None, parent: Some(&closed.id) };
    assert!(create(&hub, Actor::User, &orphan).unwrap_err().to_string().contains("parent task is done"));
}

#[tokio::test]
async fn a_checker_sends_work_back_twice_at_most() {
    let hub = hub();
    let task = new_task(&hub, "Write the guide", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    for round in 1..=2 {
        finish(&hub, &task.id, "ada", "Wrote it").await.unwrap();
        assert!(flow::review(&hub, &task.id, "ada", true, "").is_err(), "the claimer doesn't check its own work");
        assert!(flow::review(&hub, &task.id, "bo", false, " ").unwrap_err().to_string().contains("what to fix"));
        let back = flow::review(&hub, &task.id, "bo", false, "Step 3 is missing").unwrap();
        assert_eq!(
            (back.status, back.claimed_by.as_deref(), back.send_backs),
            (TaskStatus::Claimed, Some("ada"), round)
        );
        assert_eq!(back.notes.last().unwrap().text, "Sent back: Step 3 is missing");
    }
    let done = finish(&hub, &task.id, "ada", "Added step 3").await.unwrap();
    assert_eq!(done.status, TaskStatus::Done, "sent back twice: the next finish counts");

    // Their sheets count what each did, and a class sets where the abilities start.
    sheet::set_class(&hub, "bo", Some(sheet::BotClass::MartialArtist)).unwrap();
    assert!(sheet::set_class(&hub, "nobody", None).is_err());
    let sheets = sheet::sheets(&hub).unwrap();
    let of = |bot: &str| sheets.iter().find(|s| s["botId"] == bot).unwrap().clone();
    let (ada, bo) = (of("ada"), of("bo"));
    assert_eq!(
        (&ada["deeds"]["finished"], &ada["deeds"]["firstTry"], &ada["deeds"]["sentBack"]),
        (&json!(1), &json!(0), &json!(2))
    );
    assert_eq!(
        (&bo["deeds"]["checked"], &bo["deeds"]["caught"], &bo["class"]),
        (&json!(2), &json!(2), &json!("martialArtist"))
    );
    assert_eq!((&ada["class"], &ada["level"], &ada["xp"]), (&json!(null), &json!(1), &json!(10)));
    assert_eq!(bo["abilities"]["insight"], 85 + 10, "a martial artist's insight, grown by four deeds");
}

#[tokio::test]
async fn the_user_can_hand_a_planned_task_to_another_bot() {
    let hub = hub();
    let task = new_task(&hub, "Fix the login", None);
    flow::plan(&hub, &task.id, false).unwrap();
    let lead = get(&hub.store, &task.id).unwrap().claimed_by.unwrap();
    let other = if lead == "ada" { "bo" } else { "ada" };
    // No actor runs in tests: telling the bot fails once the task is claimed for it.
    assert_eq!(assign(&hub, &task.id, other).await.unwrap_err().to_string(), "unknown bot");
    assert_eq!(get(&hub.store, &task.id).unwrap().claimed_by.as_deref(), Some(other));
}
