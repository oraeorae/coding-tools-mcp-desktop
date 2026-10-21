use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::workspace::WorkspaceProfile;

const MAX_HISTORY_FILES: usize = 200;
const MAX_HISTORY_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOPICS: usize = 500;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchEvidence {
    pub kind: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchTopic {
    pub id: String,
    pub title: String,
    pub session_title: String,
    pub timestamp_ms: u64,
    pub state: String,
    pub summary: String,
    pub conclusions: Vec<String>,
    pub evidence: Vec<ResearchEvidence>,
    pub limitations: Vec<String>,
    pub source_path: String,
    pub source_number: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchHistory {
    pub topics: Vec<ResearchTopic>,
    pub scanned_files: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

fn profile_by_id(state: &AppState, id: &str) -> AppResult<WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

fn unix_value(value: Option<&Value>) -> u64 {
    let raw = value.and_then(Value::as_str).unwrap_or_default();
    raw.strip_prefix("unix:")
        .unwrap_or(raw)
        .parse::<u64>()
        .unwrap_or_default()
}

fn markdown_unix_field(content: &str, field: &str) -> u64 {
    let prefix = format!("**{field}:** unix:");
    content
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or_default()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn json_blocks(content: &str) -> Vec<Value> {
    let mut blocks = Vec::new();
    let mut in_json = false;
    let mut buffer = String::new();
    for line in content.lines() {
        if !in_json && line.trim() == "```json" {
            in_json = true;
            buffer.clear();
            continue;
        }
        if in_json && line.trim() == "```" {
            if let Ok(value) = serde_json::from_str::<Value>(&buffer) {
                blocks.push(value);
            }
            in_json = false;
            buffer.clear();
            continue;
        }
        if in_json {
            buffer.push_str(line);
            buffer.push('\n');
        }
    }
    blocks
}

fn explicit_pass(tests: &[String]) -> bool {
    tests.iter().any(|item| {
        let upper = item.to_ascii_uppercase();
        (upper.contains("PASS") || item.contains("通过"))
            && !item.contains("未通过")
            && !item.contains("未运行")
    })
}

fn session_title(content: &str) -> String {
    content
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("# "))
        .map(|line| {
            line.split_once('：')
                .map(|(_, title)| title)
                .unwrap_or(line)
                .trim()
                .to_string()
        })
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "研究记录".to_string())
}

fn evidence_from(kind: &str, label: &str, items: Vec<String>) -> Vec<ResearchEvidence> {
    items
        .into_iter()
        .map(|detail| ResearchEvidence {
            kind: kind.to_string(),
            label: label.to_string(),
            detail,
        })
        .collect()
}

fn parse_history_document(content: &str, source_number: u64) -> Vec<ResearchTopic> {
    let title = session_title(content);
    let source_path = format!("docs/history-session/{source_number}.md");
    let fallback_timestamp = markdown_unix_field(content, "Updated")
        .max(markdown_unix_field(content, "Created"));
    let blocks = json_blocks(content);
    let initial_input = blocks
        .iter()
        .find(|block| block.get("turn_id").is_none())
        .and_then(|block| block.get("raw_user_input"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut topics = Vec::new();
    for (index, block) in blocks
        .iter()
        .filter(|block| block.get("turn_id").is_some())
        .enumerate()
    {
        let findings = string_array(block.get("findings"));
        let decisions = string_array(block.get("decisions"));
        let tests = string_array(block.get("tests"));
        let runtime = string_array(block.get("runtime_state"));
        let files = string_array(block.get("files_changed"));
        let limitations = string_array(block.get("remaining_issues"));
        let notes = block
            .get("notes")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(|note| vec![note.to_string()])
            .unwrap_or_default();

        let mut evidence = Vec::new();
        evidence.extend(evidence_from("finding", "研究发现", findings.clone()));
        evidence.extend(evidence_from("test", "验证记录", tests.clone()));
        evidence.extend(evidence_from("runtime", "运行状态", runtime));
        evidence.extend(evidence_from("file", "相关文件", files));
        evidence.extend(evidence_from("note", "补充说明", notes));

        let state = if !limitations.is_empty() {
            "needs_review"
        } else if explicit_pass(&tests) && !decisions.is_empty() {
            "verified"
        } else if !evidence.is_empty() || !decisions.is_empty() {
            "partial"
        } else {
            "research"
        };
        let topic_title = block
            .get("user_intent")
            .and_then(Value::as_str)
            .or_else(|| block.get("raw_user_input").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&title)
            .to_string();
        let summary = decisions
            .first()
            .or_else(|| findings.first())
            .cloned()
            .unwrap_or_else(|| "尚未形成明确结论。".to_string());
        let timestamp = unix_value(block.get("timestamp")).max(fallback_timestamp);
        let turn_id = block
            .get("turn_id")
            .and_then(Value::as_str)
            .unwrap_or("turn");

        topics.push(ResearchTopic {
            id: format!("{source_number}-{turn_id}-{index}"),
            title: topic_title,
            session_title: title.clone(),
            timestamp_ms: timestamp.saturating_mul(1000),
            state: state.to_string(),
            summary,
            conclusions: decisions,
            evidence,
            limitations,
            source_path: source_path.clone(),
            source_number,
        });
    }

    if topics.is_empty() {
        topics.push(ResearchTopic {
            id: format!("{source_number}-session"),
            title: if initial_input.is_empty() {
                title.clone()
            } else {
                initial_input
            },
            session_title: title,
            timestamp_ms: fallback_timestamp.saturating_mul(1000),
            state: "research".to_string(),
            summary: "当前历史只有研究主题，尚未保存结论或验证证据。".to_string(),
            conclusions: Vec::new(),
            evidence: Vec::new(),
            limitations: vec!["等待后续检查点补充结论和证据。".to_string()],
            source_path,
            source_number,
        });
    }
    topics
}

fn numeric_markdown_files(history_dir: &Path) -> AppResult<Vec<(u64, PathBuf)>> {
    if !history_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(history_dir)? {
        let entry = entry?;
        let path = entry.path();
        let number = path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| value.parse::<u64>().ok());
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            if let Some(number) = number {
                files.push((number, path));
            }
        }
    }
    files.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(files)
}

fn read_research_history_from_root(root: &Path) -> AppResult<ResearchHistory> {
    let history_dir = root.join("docs/history-session");
    if !history_dir.exists() {
        return Ok(ResearchHistory {
            topics: Vec::new(),
            scanned_files: 0,
            truncated: false,
            warnings: Vec::new(),
        });
    }
    let canonical_root = root.canonicalize()?;
    let canonical_history = history_dir.canonicalize()?;
    if !canonical_history.starts_with(&canonical_root) {
        return Err(AppError::Message(
            "history directory resolves outside the workspace".to_string(),
        ));
    }

    let files = numeric_markdown_files(&canonical_history)?;
    let truncated = files.len() > MAX_HISTORY_FILES;
    let selected: Vec<_> = files.into_iter().take(MAX_HISTORY_FILES).collect();
    let scanned_files = selected.len();
    let mut topics = Vec::new();
    let mut warnings = Vec::new();
    for (number, path) in selected {
        let canonical_path = match path.canonicalize() {
            Ok(path) if path.starts_with(&canonical_history) => path,
            Ok(_) => {
                warnings.push(format!(
                    "跳过工作区外的历史文件：docs/history-session/{number}.md"
                ));
                continue;
            }
            Err(error) => {
                warnings.push(format!(
                    "无法定位 docs/history-session/{number}.md：{error}"
                ));
                continue;
            }
        };
        let metadata = fs::metadata(&canonical_path)?;
        if metadata.len() > MAX_HISTORY_FILE_BYTES {
            warnings.push(format!("跳过过大的历史文件：docs/history-session/{number}.md"));
            continue;
        }
        match fs::read_to_string(&canonical_path) {
            Ok(content) => topics.extend(parse_history_document(&content, number)),
            Err(error) => warnings.push(format!(
                "无法读取 docs/history-session/{number}.md：{error}"
            )),
        }
        if topics.len() >= MAX_TOPICS {
            topics.truncate(MAX_TOPICS);
            break;
        }
    }
    topics.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| right.source_number.cmp(&left.source_number))
    });

    Ok(ResearchHistory {
        topics,
        scanned_files,
        truncated,
        warnings,
    })
}

#[tauri::command]
pub async fn read_research_history(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<ResearchHistory> {
    let profile = profile_by_id(&state, &id)?;
    let root = PathBuf::from(profile.path);
    tauri::async_runtime::spawn_blocking(move || read_research_history_from_root(&root))
        .await
        .map_err(|error| AppError::Message(format!("research history worker failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_conclusions_evidence_and_review_state() {
        let input = r#"# 会话 7：策略稳定性复核

**Created:** unix:1700000000
**Updated:** unix:1700000060

```json
{"raw_user_input":"检查策略稳定性","captured_at":"unix:1700000000"}
```

```json
{
  "turn_id":"turn-1",
  "timestamp":"unix:1700000060",
  "user_intent":"检查策略稳定性",
  "findings":["最大回撤扩大"],
  "decisions":["暂不晋级"],
  "tests":["合成测试 4 项通过"],
  "runtime_state":["任务已结束"],
  "remaining_issues":["仍缺样本外验证"],
  "files_changed":["reports/result.json"]
}
```
"#;
        let topics = parse_history_document(input, 7);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].state, "needs_review");
        assert_eq!(topics[0].summary, "暂不晋级");
        assert_eq!(topics[0].timestamp_ms, 1_700_000_060_000);
        assert_eq!(topics[0].evidence.len(), 4);
        assert_eq!(topics[0].source_path, "docs/history-session/7.md");
    }

    #[test]
    fn preserves_topic_without_a_checkpoint() {
        let input = r#"# 会话 8：新主题

**Created:** unix:1700000100
**Updated:** unix:1700000100

```json
{"raw_user_input":"研究新的轮动逻辑","captured_at":"unix:1700000100"}
```
"#;
        let topics = parse_history_document(input, 8);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].state, "research");
        assert_eq!(topics[0].title, "研究新的轮动逻辑");
        assert!(topics[0].evidence.is_empty());
    }

    #[test]
    fn reads_only_numeric_markdown_history() {
        let root = tempfile::tempdir().expect("temp root");
        let history = root.path().join("docs/history-session");
        fs::create_dir_all(&history).expect("history dir");
        fs::write(
            history.join("1.md"),
            "# 会话 1：主题\n\n**Created:** unix:1700000200\n**Updated:** unix:1700000200\n",
        )
        .expect("write history");
        fs::write(history.join("README.md"), "ignored").expect("write readme");
        let result = read_research_history_from_root(root.path()).expect("read research");
        assert_eq!(result.scanned_files, 1);
        assert_eq!(result.topics.len(), 1);
        assert!(!result.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn skips_numeric_symlink_that_escapes_workspace() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temp root");
        let outside = tempfile::tempdir().expect("outside root");
        let history = root.path().join("docs/history-session");
        fs::create_dir_all(&history).expect("history dir");
        let outside_file = outside.path().join("private.md");
        fs::write(&outside_file, "# should not be read").expect("outside file");
        symlink(&outside_file, history.join("1.md")).expect("history symlink");

        let result = read_research_history_from_root(root.path()).expect("read research");
        assert!(result.topics.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("工作区外"));
    }
}
