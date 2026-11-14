use super::*;

fn new_plan_task(hub: &Hub) -> Task {
    let new = NewTask {
        title: "Make a poster",
        detail: "",
        project: None,
        parent: None,
        acceptance: vec![],
        approve_plan: true,
        after: vec![],
    };
    create(hub, Actor::User, &new).unwrap()
}

#[tokio::test]
async fn the_claimer_asks_the_user_before_planning_and_is_told_the_answers() {
    let hub = hub();
    let task = new_plan_task(&hub);
    let lead = flow::plan(&hub, &task.id, false).unwrap()["botId"].as_str().unwrap().to_owned();
    let other = if lead == "ada" { "bo" } else { "ada" };

    let too_many = json!(["a", "b", "c", "d"]);
    assert!(brief::ask(&hub, &task.id, &lead, &too_many).unwrap_err().to_string().contains("1 to 3"));
    assert!(brief::ask(&hub, &task.id, &lead, &json!([])).is_err(), "at least one question");
    assert!(brief::ask(&hub, &task.id, &lead, &json!(["two\nlines"])).is_err(), "one line each");
    assert!(brief::ask(&hub, &task.id, &lead, &json!(["x".repeat(301)])).is_err(), "at most 300 bytes");
    assert!(brief::ask(&hub, &task.id, other, &json!(["Whom?"])).is_err(), "only the claimer asks");

    let asked = brief::ask(&hub, &task.id, &lead, &json!(["For whom?", "What size?"])).unwrap();
    assert_eq!(asked.approval, brief::Approval::Asking);
    assert_eq!(asked.questions.len(), 2);
    assert!(asked.questions.iter().all(|q| q.answer.is_none()));

    // Until the user answers: no work, no subtasks, no plan.
    let early = finish(&hub, &task.id, &lead, "Did it").await.unwrap_err().to_string();
    assert!(early.contains("hasn't answered"), "{early}");
    let part = NewTask {
        title: "Print it",
        detail: "",
        project: None,
        parent: Some(&task.id),
        acceptance: vec![],
        approve_plan: false,
        after: vec![],
    };
    assert!(create(&hub, Actor::Bot(&lead), &part).is_err(), "a subtask waits for the answers");
    assert!(brief::propose(&hub, &task.id, &lead, "Plan", &json!(["x"])).is_err(), "no plan before the answers");
    let order = brief::work_order(&get(&hub.store, &task.id).unwrap());
    assert!(order.contains("don't start until you're told"), "{order}");

    // The answers match the questions one for one.
    let short = brief::answer(&hub, &task.id, &json!(["Kids"])).unwrap_err().to_string();
    assert!(short.contains("2 questions"), "{short}");
    assert_eq!(get(&hub.store, &task.id).unwrap().approval, brief::Approval::Asking);

    let answered = brief::answer(&hub, &task.id, &json!(["  Kids  ", ""])).unwrap();
    assert_eq!(answered.approval, brief::Approval::Waiting);
    assert_eq!(answered.questions[0].answer.as_deref(), Some("Kids"), "trimmed");
    assert_eq!(answered.questions[1].answer.as_deref(), Some(""), "an empty answer is kept");
    let order = brief::work_order(&answered);
    assert!(order.contains("Q: For whom?\nA: Kids") && !order.contains("ask_user"), "{order}");
    assert!(brief::answer(&hub, &task.id, &json!(["a", "b"])).is_err(), "nothing waits for answers now");

    // Asked once: a second round is refused, and the plan goes ahead.
    let again = brief::ask(&hub, &task.id, &lead, &json!(["More?"])).unwrap_err().to_string();
    assert_eq!(again, "you already asked: propose_plan now");
    let proposed = brief::propose(&hub, &task.id, &lead, "Two sizes, for kids", &json!(["Printed A2"])).unwrap();
    assert_eq!(proposed.approval, brief::Approval::Proposed);
}

#[test]
fn an_unclear_task_gets_the_ask_guidance_before_the_plan() {
    let hub = hub();
    let task = new_plan_task(&hub);
    flow::plan(&hub, &task.id, false).unwrap();
    let order = brief::work_order(&get(&hub.store, &task.id).unwrap());
    assert!(order.contains(&format!("first ask_user {} up to 3 short questions", task.id)), "{order}");
    assert!(order.contains(&format!("propose_plan {}", task.id)), "{order}");
}

#[tokio::test]
async fn reopening_a_task_forgets_its_questions() {
    let hub = hub();
    let task = new_plan_task(&hub);
    let lead = flow::plan(&hub, &task.id, false).unwrap()["botId"].as_str().unwrap().to_owned();
    brief::ask(&hub, &task.id, &lead, &json!(["For whom?"])).unwrap();
    let reopened = set_status(&hub, &task.id, TaskStatus::Open).await.unwrap();
    assert_eq!((reopened.approval, reopened.questions.len()), (brief::Approval::Waiting, 0));
}
