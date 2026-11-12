use super::*;

#[test]
fn a_saved_task_is_a_template_listed_by_name_and_deleted() {
    let hub = hub();
    let new = NewTask {
        title: "Make a poster",
        detail: "A3, two colours",
        project: None,
        parent: None,
        acceptance: vec!["Fits A3".into()],
        approve_plan: true,
        after: vec![],
    };
    let task = create(&hub, Actor::User, &new).unwrap();
    let saved = template::save(&hub, &task.id, None).unwrap();
    assert_eq!(
        (saved.name.as_str(), saved.title.as_str(), saved.detail.as_str()),
        ("Make a poster", "Make a poster", "A3, two colours")
    );
    assert_eq!(
        (saved.acceptance.clone(), saved.approve_plan, saved.project.clone()),
        (vec!["Fits A3".to_owned()], true, None)
    );

    let named = template::save(&hub, &task.id, Some("  Poster  ")).unwrap();
    let listed: Vec<String> = template::list(&hub.store).into_iter().map(|t| t.name).collect();
    assert_eq!(listed, ["Make a poster", "Poster"], "by name");

    template::delete(&hub, &saved.id).unwrap();
    assert_eq!(template::list(&hub.store), vec![named]);
    assert!(template::delete(&hub, &saved.id).is_err(), "already deleted");
    assert!(template::delete(&hub, "not-an-id").unwrap_err().to_string().contains("invalid template id"));
    assert!(template::save(&hub, "not-a-task", None).is_err());
    assert!(template::save(&hub, &task.id, Some("two\nlines")).is_err(), "the name follows the title rules");
}

#[test]
fn a_task_that_needs_no_approval_saves_a_template_that_starts_at_once() {
    let hub = hub();
    let task = new_task(&hub, "Tidy the folder", None);
    let saved = template::save(&hub, &task.id, None).unwrap();
    assert!(!saved.approve_plan);
}

#[test]
fn the_user_keeps_at_most_50_templates() {
    let hub = hub();
    let task = new_task(&hub, "Write", None);
    for i in 0..50 {
        template::save(&hub, &task.id, Some(format!("T{i}").as_str())).unwrap();
    }
    let full = template::save(&hub, &task.id, Some("one more")).unwrap_err().to_string();
    assert!(full.contains("50 templates"), "{full}");
    assert_eq!(template::list(&hub.store).len(), 50);
}
