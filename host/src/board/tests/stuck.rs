use super::*;
use crate::board::stale::{self, STALE};
use crate::hub::BotStatus;
use std::time::{Duration, Instant};

fn limit_ms() -> i64 {
    i64::try_from(STALE.as_millis()).unwrap()
}

#[tokio::test]
async fn a_task_is_stale_only_when_it_sat_unchanged_and_nothing_waits_on_the_user() {
    let hub = hub();
    let task = new_task(&hub, "Write the parser", None);
    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    let base = claimed.updated_at;

    assert!(!stale::stale(&claimed, false, base + limit_ms()), "exactly STALE is not yet older than it");
    assert!(stale::stale(&claimed, false, base + limit_ms() + 1));
    assert!(!stale::stale(&claimed, true, base + limit_ms() + 1), "a coordinator waits for its parts");

    let mut proposed = claimed.clone();
    proposed.approval = brief::Approval::Proposed;
    assert!(!stale::stale(&proposed, false, base + limit_ms() + 1), "a plan waiting for the user is never stale");
    let mut asking = claimed.clone();
    asking.approval = brief::Approval::Asking;
    assert!(!stale::stale(&asking, false, base + limit_ms() + 1), "questions waiting for the user are never stale");
    let mut open = claimed.clone();
    open.status = TaskStatus::Open;
    assert!(!stale::stale(&open, false, base + limit_ms() + 1), "only a claimed task can be stale");
}

#[tokio::test]
async fn a_stuck_claimer_is_nudged_once_then_its_task_goes_back_to_the_board() {
    let hub = hub();
    let task = new_task(&hub, "Write the parser", None);
    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    let late = claimed.updated_at + limit_ms() + 1;
    let now = Instant::now();

    stale::sweep(&hub, late, now);
    assert!(hub.board.stale.0.locked().contains_key(&task.id), "nudged");
    assert_eq!(get(&hub.store, &task.id).unwrap().status, TaskStatus::Claimed);

    stale::sweep(&hub, late, now + Duration::from_secs(60));
    assert_eq!(get(&hub.store, &task.id).unwrap().status, TaskStatus::Claimed, "not yet STALE since the nudge");

    stale::sweep(&hub, late, now + STALE + Duration::from_secs(1));
    let released = get(&hub.store, &task.id).unwrap();
    assert_eq!((released.status, released.claimed_by.clone()), (TaskStatus::Open, None));
    assert_eq!(released.notes.last().unwrap().text, "Released: no progress for a while");
    assert!(!hub.board.stale.0.locked().contains_key(&task.id), "the entry is dropped");
}

#[tokio::test]
async fn a_task_that_moves_again_is_forgotten_and_a_busy_claimer_is_left_alone() {
    let hub = hub();
    let task = new_task(&hub, "Write the parser", None);
    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    let late = claimed.updated_at + limit_ms() + 1;
    let now = Instant::now();

    stale::sweep(&hub, late, now);
    assert!(hub.board.stale.0.locked().contains_key(&task.id));
    // The claimer notes its progress: the task moves, so it isn't stale any more.
    let moved = note(&hub, &task.id, Actor::Bot("ada"), "halfway").unwrap();
    stale::sweep(&hub, moved.updated_at + 1, now + STALE + Duration::from_secs(1));
    assert!(!hub.board.stale.0.locked().contains_key(&task.id), "forgotten once it moves");
    assert_eq!(get(&hub.store, &task.id).unwrap().status, TaskStatus::Claimed);

    // A claimer still in a turn is neither nudged nor released.
    let late = moved.updated_at + limit_ms() + 1;
    hub.set_runtime("ada", |r| r.status = BotStatus::Working);
    stale::sweep(&hub, late, now + STALE * 3);
    assert!(!hub.board.stale.0.locked().contains_key(&task.id), "busy: not nudged");
    assert_eq!(get(&hub.store, &task.id).unwrap().status, TaskStatus::Claimed);
}

#[tokio::test]
async fn a_coordinator_with_open_parts_is_never_poked() {
    let hub = hub();
    let parent = new_task(&hub, "Build the site", None);
    claim(&hub, &parent.id, "ada").await.unwrap();
    let part = NewTask {
        title: "The footer",
        detail: "",
        project: None,
        parent: Some(&parent.id),
        acceptance: vec![],
        approve_plan: false,
        after: vec![],
    };
    create(&hub, Actor::Bot("ada"), &part).unwrap();
    let claimed = get(&hub.store, &parent.id).unwrap();
    stale::sweep(&hub, claimed.updated_at + limit_ms() + 1, Instant::now());
    assert!(!hub.board.stale.0.locked().contains_key(&parent.id), "it waits for its parts");
    assert_eq!(get(&hub.store, &parent.id).unwrap().status, TaskStatus::Claimed);
}
