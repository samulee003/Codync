use super::*;

fn turns(hub: &Hub, id: &str) -> u32 {
    get(&hub.store, id).unwrap().turns
}

#[tokio::test]
async fn the_first_claim_starts_the_clock_and_later_claims_keep_it() {
    let hub = hub();
    let task = new_task(&hub, "Clock", None);
    assert_eq!(task.started_at, None);
    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    assert!(claimed.started_at.is_some(), "the first claim starts it");

    change(&hub, &task.id, |t| {
        t.started_at = Some(1);
        Ok(())
    })
    .unwrap();
    release(&hub, &task.id, Actor::Bot("ada"), None).unwrap();
    let again = claim(&hub, &task.id, "bo").await.unwrap();
    assert_eq!(again.started_at, Some(1), "a later claim keeps the first one");
}

#[tokio::test]
async fn accepted_work_stamps_when_it_finished() {
    let hub = hub();
    let task = new_task(&hub, "Finish me", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    let checking = finish(&hub, &task.id, "ada", "Done it").await.unwrap();
    assert_eq!(checking.finished_at, None, "not finished while another bot checks it");
    let done = review(&hub, &task.id, "bo", true, "").unwrap();
    assert_eq!(done.status, TaskStatus::Done);
    assert!(done.finished_at.is_some());
}

#[tokio::test]
async fn each_prompt_about_a_task_is_a_turn_for_it() {
    let hub = hub();
    let task = new_task(&hub, "Write the guide", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    assert_eq!(turns(&hub, &task.id), 0, "claiming is not a turn");

    finish(&hub, &task.id, "ada", "Wrote it").await.unwrap();
    assert_eq!(turns(&hub, &task.id), 1, "the checker's prompt");
    review(&hub, &task.id, "bo", false, "Step 3 is missing").unwrap();
    assert_eq!(turns(&hub, &task.id), 2, "the send-back to the claimer");
    finish(&hub, &task.id, "ada", "Added step 3").await.unwrap();
    assert_eq!(turns(&hub, &task.id), 3, "the second check");
    review(&hub, &task.id, "bo", true, "").unwrap();
    assert_eq!(turns(&hub, &task.id), 3, "an accepted task prompts no one");
}

#[tokio::test]
async fn counting_a_turn_leaves_the_task_unchanged_for_staleness() {
    let hub = hub();
    let task = new_task(&hub, "Stay quiet", None);
    let claimed = claim(&hub, &task.id, "ada").await.unwrap();
    effort::count_turn(&hub, &task.id);
    let counted = get(&hub.store, &task.id).unwrap();
    assert_eq!((counted.turns, counted.updated_at), (1, claimed.updated_at), "a nudge doesn't reset the clock");
}

#[tokio::test]
async fn reopening_a_task_clears_its_effort() {
    let hub = hub();
    let task = new_task(&hub, "Do it twice", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    finish(&hub, &task.id, "ada", "Done").await.unwrap();
    review(&hub, &task.id, "bo", true, "").unwrap();
    let done = get(&hub.store, &task.id).unwrap();
    assert!(done.started_at.is_some() && done.finished_at.is_some() && done.turns > 0);

    let reopened = set_status(&hub, &task.id, TaskStatus::Open).await.unwrap();
    assert_eq!((reopened.started_at, reopened.finished_at, reopened.turns), (None, None, 0));
}
