use super::*;

#[tokio::test]
async fn a_send_back_is_a_lesson_the_claimer_is_given_next() {
    let hub = hub();
    let task = new_task(&hub, "Write the guide", None);
    claim(&hub, &task.id, "ada").await.unwrap();
    finish(&hub, &task.id, "ada", "Wrote it").await.unwrap();
    review(&hub, &task.id, "bo", false, "Step 3 is missing").unwrap();

    let got = lessons::recent(&hub, "ada", lessons::QUOTED);
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].text.as_str(), got[0].task.as_str()), ("Step 3 is missing", "Write the guide"));
    assert_eq!(lessons::recent(&hub, "bo", lessons::QUOTED), Vec::new(), "the checker learns nothing from it");

    // The next task the claimer is given quotes it: in the claim's answer, in its assignment and in its planning.
    let next = new_task(&hub, "Write the poster", None);
    let claimed = tools::call(&hub, "ada", "claim_task", &json!({"taskId": next.id})).await.unwrap();
    assert_eq!(claimed["lessons"], json!(["Step 3 is missing"]));
    let assigned = manage::assigned_prompt(&next, "ADA", &lessons::prompt(&hub, "ada"));
    assert!(
        assigned.ends_with("\n\nLessons from earlier checks of your work (don't repeat these):\n- Step 3 is missing")
    );
    let planned = flow::plan_prompt(&next, false, &lessons::prompt(&hub, "ada"), "Teammates:");
    assert!(planned.contains("- Step 3 is missing") && planned.ends_with("Teammates:"));

    let quiet = new_task(&hub, "Fix the sign", None);
    let plain = tools::call(&hub, "bo", "claim_task", &json!({"taskId": quiet.id})).await.unwrap();
    assert!(plain.get("lessons").is_none(), "no lessons, no field");
}

#[test]
fn a_bot_keeps_its_last_20_lessons_and_quotes_5() {
    let hub = hub();
    for i in 0..25 {
        lessons::record(&hub, "ada", "Write", &format!("lesson {i}"));
    }
    let kept = lessons::recent(&hub, "ada", usize::MAX);
    assert_eq!(kept.len(), lessons::MAX_LESSONS);
    assert_eq!((kept[0].text.as_str(), kept[19].text.as_str()), ("lesson 24", "lesson 5"), "newest first");
    let prompt = lessons::prompt(&hub, "ada");
    assert!(prompt.contains("- lesson 20") && !prompt.contains("- lesson 19"));
    assert_eq!(prompt.matches("\n- ").count(), lessons::QUOTED);
}

#[test]
fn empty_feedback_keeps_no_lesson_and_forgetting_clears_them() {
    let hub = hub();
    lessons::record(&hub, "ada", "Write", "  \n ");
    assert_eq!(lessons::recent(&hub, "ada", lessons::QUOTED), Vec::new());

    lessons::record(&hub, "ada", "Write", "Cite  the\nsource");
    assert_eq!(lessons::recent(&hub, "ada", lessons::QUOTED)[0].text, "Cite the source", "one line");
    lessons::forget(&hub, "ada").unwrap();
    assert_eq!(lessons::recent(&hub, "ada", lessons::QUOTED), Vec::new());
    assert_eq!(lessons::prompt(&hub, "ada"), "");
}
