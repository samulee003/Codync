//! Task templates: a task that went well is saved (its title, detail, criteria, project and whether its plan is approved
//! first) and kept to reuse. Kept in kv (`board.template.<id>`), at most `MAX_TEMPLATES`; saves and deletes are
//! serialized by `Board::templates`, so the cap holds.

use super::brief::Approval;
use super::{Task, title};
use crate::LockExt;
use crate::hub::Hub;
use crate::store::Store;
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

const KEY_PREFIX: &str = "board.template.";
/// Templates the user keeps.
const MAX_TEMPLATES: usize = 50;

/// A saved task: what a new task starts from.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    /// The name the user knows it by.
    pub name: String,
    pub title: String,
    pub detail: String,
    pub acceptance: Vec<String>,
    pub approve_plan: bool,
    pub project: Option<String>,
}

fn key(id: &str) -> String {
    format!("{KEY_PREFIX}{id}")
}

fn all(store: &Store) -> Vec<Template> {
    store.kv_prefix(KEY_PREFIX).into_iter().filter_map(|k| serde_json::from_str(&store.kv_get(&k)?).ok()).collect()
}

/// The saved templates, by name.
pub fn list(store: &Store) -> Vec<Template> {
    let mut templates = all(store);
    templates.sort_by_key(|t| t.name.to_lowercase());
    templates
}

/// Saves a task as a template. `name` defaults to the task's title.
pub fn save(hub: &Hub, task_id: &str, name: Option<&str>) -> Result<Template> {
    let task: Task = super::get(&hub.store, task_id)?;
    let name = title(name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(task.title.as_str()))?;
    let _g = hub.board.templates.locked();
    ensure!(all(&hub.store).len() < MAX_TEMPLATES, "you have {MAX_TEMPLATES} templates: delete one first");
    let template = Template {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        title: task.title,
        detail: task.detail,
        acceptance: task.acceptance,
        approve_plan: task.approval != Approval::NotNeeded,
        project: task.project,
    };
    hub.store.kv_set(&key(&template.id), &serde_json::to_string(&template)?)?;
    Ok(template)
}

/// Deletes a saved template.
pub fn delete(hub: &Hub, id: &str) -> Result<()> {
    let id = uuid::Uuid::parse_str(id).context("invalid template id")?.to_string();
    let _g = hub.board.templates.locked();
    ensure!(hub.store.kv_get(&key(&id)).is_some(), "no template {id}");
    hub.store.kv_delete(&key(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_is_written_in_camel_case() {
        let template = Template {
            id: "1".into(),
            name: "Poster".into(),
            title: "Make a poster".into(),
            detail: String::new(),
            acceptance: vec!["A4".into()],
            approve_plan: true,
            project: None,
        };
        let wire = serde_json::to_value(&template).unwrap();
        assert_eq!(wire["approvePlan"], true);
        assert_eq!(wire["acceptance"][0], "A4");
        assert!(wire["project"].is_null());
    }
}
