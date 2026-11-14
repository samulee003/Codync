//! An alternative computer-use driver: agent-desktop (Apache-2.0,
//! <https://github.com/vercel-labs/agent-desktop>), chosen with `CODYNC_COMPUTER_DRIVER=agent-desktop`.
//! It reads an app's accessibility tree (macOS AX, Windows UI Automation, Linux AT-SPI) and acts on
//! the refs of that snapshot, headless by default: no pointer movement and no focus change.
//!
//! Every call runs the `agent-desktop` CLI once and reads its JSON envelope
//! (`{ok, data | error}`). Each bot gets its own `--session`, so one bot's refs never resolve
//! for another. Typed text goes through stdin, never through the process arguments.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// The most one call may take: the MCP bridge gives up at 60 s.
const CALL_TIMEOUT: Duration = Duration::from_secs(55);
/// The longest result text handed to an agent; snapshots of dense apps can be huge.
const MAX_TEXT: usize = 100_000;
const DRIVER_ENV: &str = "CODYNC_COMPUTER_DRIVER";

/// The tools bots get. Refs come from `desktop_snapshot` and belong to it: take a new snapshot
/// when a call answers `STALE_REF` or `AMBIGUOUS_TARGET`.
static TOOLS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    let app = json!({"type": "string", "description": "Application name."});
    let target = json!({"type": "string", "description": "A ref from desktop_snapshot, like @s8f3k2p9:e8."});
    vec![
        tool("desktop_list_apps", "List the running GUI apps.", true, &json!({"app": app}), &[]),
        tool(
            "desktop_list_windows",
            "List the visible windows, each with a window_id for desktop_snapshot.",
            true,
            &json!({"app": app}),
            &[],
        ),
        tool(
            "desktop_snapshot",
            "Read an app's accessibility tree as JSON; every element you can act on has a ref. Pass `window_id` when the app has several windows.",
            true,
            &json!({"app": app, "window_id": {"type": "string"}, "interactive_only": {"type": "boolean", "description": "Only elements you can act on (default true)."}}),
            &["app"],
        ),
        tool("desktop_click", "Click a ref.", false, &json!({"ref": target}), &["ref"]),
        tool(
            "desktop_type",
            "Insert text into a text field ref.",
            false,
            &json!({"ref": target, "text": {"type": "string"}}),
            &["ref", "text"],
        ),
        tool(
            "desktop_press",
            "Send a key combo (return, escape, cmd+c, shift+tab…) to an app.",
            false,
            &json!({"combo": {"type": "string"}, "app": app, "window_id": {"type": "string"}}),
            &["combo", "app"],
        ),
        tool(
            "desktop_scroll",
            "Scroll a ref.",
            false,
            &json!({"ref": target, "direction": {"type": "string", "enum": ["up", "down", "left", "right"]}, "amount": {"type": "integer", "minimum": 1, "maximum": 50}}),
            &["ref"],
        ),
        tool("desktop_launch", "Launch an app by name or bundle id.", false, &json!({"app": app}), &["app"]),
    ]
});

/// Whether the host's environment asks for this driver (`CODYNC_COMPUTER_DRIVER`), whatever the setting says.
pub(super) fn forced() -> bool {
    std::env::var(DRIVER_ENV).is_ok_and(|v| v.eq_ignore_ascii_case("agent-desktop"))
}

/// The CLI installed beside the host, else the first one on `PATH`.
pub(super) fn executable() -> Option<PathBuf> {
    let name = if cfg!(windows) { "agent-desktop.exe" } else { "agent-desktop" };
    let beside = std::env::current_exe().ok()?.parent().map(|dir| dir.join(name)).filter(|p| p.is_file());
    beside.or_else(|| {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| p.is_file())
    })
}

fn tool(name: &str, description: &str, read_only: bool, properties: &Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": !read_only, "openWorldHint": true},
    })
}

pub(super) fn tools() -> Vec<Value> {
    TOOLS.clone()
}

pub(super) fn offered(name: &str) -> bool {
    TOOLS.iter().any(|t| t["name"] == name)
}

pub(super) fn read_only(name: &str) -> bool {
    TOOLS.iter().any(|t| t["name"] == name && t["annotations"]["readOnlyHint"] == true)
}

/// A string argument that can't be read as a flag.
fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    let v = args[key].as_str().filter(|v| !v.is_empty()).ok_or_else(|| anyhow!("`{key}` is required"))?;
    if v.starts_with('-') {
        bail!("`{key}` can't start with `-`");
    }
    Ok(v)
}

fn opt<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>> {
    if args[key].is_null() { Ok(None) } else { text(args, key).map(Some) }
}

fn target(args: &Value) -> Result<&str> {
    let r = text(args, "ref")?;
    if !r.starts_with('@') {
        bail!("`ref` must be a ref from desktop_snapshot, like @s8f3k2p9:e8");
    }
    Ok(r)
}

/// The CLI arguments for a tool call, and the text to pipe to its stdin.
fn command(name: &str, args: &Value) -> Result<(Vec<String>, Option<String>)> {
    let mut cli_args: Vec<String> = Vec::new();
    let mut flag = |key: &str, value: Option<&str>| {
        if let Some(v) = value {
            cli_args.push(format!("--{key}={v}"));
        }
    };
    let mut stdin = None;
    let (cmd, positional) = match name {
        "desktop_list_apps" => {
            flag("app", opt(args, "app")?);
            ("list-apps", None)
        }
        "desktop_list_windows" => {
            flag("app", opt(args, "app")?);
            ("list-windows", None)
        }
        "desktop_snapshot" => {
            flag("app", Some(text(args, "app")?));
            flag("window-id", opt(args, "window_id")?);
            if args["interactive_only"] != false {
                cli_args.push("--interactive-only".into());
            }
            cli_args.push("--compact".into());
            ("snapshot", None)
        }
        "desktop_click" => ("click", Some(target(args)?)),
        "desktop_type" => {
            let t = args["text"].as_str().ok_or_else(|| anyhow!("`text` is required"))?;
            stdin = Some(t.to_owned());
            cli_args.push("--stdin".into());
            ("type", Some(target(args)?))
        }
        "desktop_press" => {
            flag("app", Some(text(args, "app")?));
            flag("window-id", opt(args, "window_id")?);
            ("press", Some(text(args, "combo")?))
        }
        "desktop_scroll" => {
            if let Some(d) = args["direction"].as_str() {
                if !["up", "down", "left", "right"].contains(&d) {
                    bail!("`direction` must be up, down, left or right");
                }
                cli_args.push(format!("--direction={d}"));
            }
            if let Some(n) = args["amount"].as_u64() {
                cli_args.push(format!("--amount={}", n.clamp(1, 50)));
            }
            ("scroll", Some(target(args)?))
        }
        "desktop_launch" => ("launch", Some(text(args, "app")?)),
        _ => bail!("unknown computer tool `{name}`"),
    };
    let mut full = vec![cmd.to_owned()];
    full.extend(cli_args);
    if let Some(p) = positional {
        full.push("--".into());
        full.push(p.to_owned());
    }
    Ok((full, stdin))
}

/// A session name the CLI accepts: letters, digits, `-` and `_`.
fn session(bot: &str) -> String {
    let id: String = bot.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(48).collect();
    format!("codync-{id}")
}

/// The CLI's envelope as the agent reads it: `{content, isError?}`.
fn agent_content(envelope: &Value) -> Value {
    let ok = envelope["ok"] == true;
    let body = if ok { &envelope["data"] } else { &envelope["error"] };
    let mut text = serde_json::to_string(body).unwrap_or_default();
    if text.len() > MAX_TEXT {
        let mut cut = MAX_TEXT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("… (cut; snapshot a smaller part of the app)");
    }
    let mut out = json!({"content": [{"type": "text", "text": text}]});
    if !ok {
        out["isError"] = true.into();
    }
    out
}

/// Runs one tool for `bot`: `{content, isError?}` for the agent.
pub(super) async fn call(bot: &str, name: &str, args: Value) -> Result<Value> {
    let (cli_args, stdin) = command(name, &args)?;
    let exe = executable().context("agent-desktop isn't installed. Install it with `npm install -g agent-desktop`.")?;
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg(format!("--session={}", session(bot)))
        .args(&cli_args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().with_context(|| format!("can't start {}", exe.display()))?;
    let output = tokio::time::timeout(CALL_TIMEOUT, async {
        if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(text.as_bytes()).await?;
        }
        child.wait_with_output().await
    })
    .await
    .map_err(|_| anyhow!("agent-desktop didn't answer in time"))?
    .context("agent-desktop failed")?;
    let envelope: Value = serde_json::from_slice(&output.stdout).context("agent-desktop sent no JSON")?;
    Ok(agent_content(&envelope))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_say_which_only_look() {
        assert!(read_only("desktop_snapshot") && read_only("desktop_list_windows"));
        assert!(offered("desktop_click") && !read_only("desktop_click") && !read_only("desktop_launch"));
        assert!(!offered("clipboard_get") && !offered("desktop_batch"));
    }

    #[test]
    fn typed_text_goes_to_stdin_not_the_arguments() {
        let (cli_args, stdin) = command("desktop_type", &json!({"ref": "@s1:e2", "text": "hunter2"})).unwrap();
        assert_eq!(cli_args, ["type", "--stdin", "--", "@s1:e2"]);
        assert_eq!(stdin.as_deref(), Some("hunter2"));
    }

    #[test]
    fn snapshots_default_to_interactive_elements() {
        let (cli_args, _) = command("desktop_snapshot", &json!({"app": "Finder"})).unwrap();
        assert_eq!(cli_args, ["snapshot", "--app=Finder", "--interactive-only", "--compact"]);
    }

    #[test]
    fn arguments_can_not_become_flags_or_bad_refs() {
        assert!(command("desktop_launch", &json!({"app": "--headed"})).is_err());
        assert!(command("desktop_snapshot", &json!({"app": "x", "window_id": "--trace"})).is_err());
        assert!(command("desktop_click", &json!({"ref": "e8"})).is_err());
        assert!(command("desktop_scroll", &json!({"ref": "@s1:e2", "direction": "sideways"})).is_err());
        assert!(command("desktop_batch", &json!({})).is_err());
    }

    #[test]
    fn sessions_are_per_bot_and_clean() {
        assert_eq!(session("a b/../c"), "codync-abc");
    }

    #[test]
    fn envelopes_become_agent_results() {
        let ok = agent_content(&json!({"ok": true, "data": {"ref_count": 1}}));
        assert_eq!(ok["content"][0]["text"], r#"{"ref_count":1}"#);
        assert!(ok.get("isError").is_none());
        let bad = agent_content(&json!({"ok": false, "error": {"code": "STALE_REF"}}));
        assert_eq!(bad["isError"], true);
        assert!(bad["content"][0]["text"].as_str().unwrap().contains("STALE_REF"));
    }
}
