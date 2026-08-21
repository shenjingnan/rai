/// dsh 桥事件：deepseek-harness 插件推送到 `/dsh/events` 的语义化任务事件。
use serde::{Deserialize, Serialize};

/// 任务事件（序列化为 kebab-case `type` 判别字段，与前端 `DshEventInfo` 对应）。
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DshEvent {
    /// 会话 idle → running（dsh `agent/status`）
    TaskStarted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// turn 结束且 reason.kind = completed
    TaskFinished {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// turn 结束且 reason.kind = error
    TaskFailed {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// turn 结束且 reason.kind 为 aborted/interrupted/max-tokens/blocked 等
    TaskInterrupted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl DshEvent {
    /// 事件类型名（kebab-case，节流 key / 日志用）。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskStarted { .. } => "task-started",
            Self::TaskFinished { .. } => "task-finished",
            Self::TaskFailed { .. } => "task-failed",
            Self::TaskInterrupted { .. } => "task-interrupted",
        }
    }

    /// 会话 id（节流 key 用）。
    pub fn session_id(&self) -> &str {
        match self {
            Self::TaskStarted { session_id, .. }
            | Self::TaskFinished { session_id, .. }
            | Self::TaskFailed { session_id, .. }
            | Self::TaskInterrupted { session_id, .. } => session_id,
        }
    }

    /// 任务标题（模板台词用）。
    pub fn title(&self) -> Option<&str> {
        match self {
            Self::TaskStarted { title, .. }
            | Self::TaskFinished { title, .. }
            | Self::TaskFailed { title, .. }
            | Self::TaskInterrupted { title, .. } => title.as_deref(),
        }
    }
}

/// 宽容解析用的原始载荷：全字段可缺省，未知 `type` 不报错（前向兼容）。
#[derive(Debug, Default, Deserialize)]
struct RawEvent {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    detail: Option<String>,
}

/// 解析一条事件载荷。
///
/// - 非法 JSON / 非 JSON 对象 → `Err`（HTTP 层回 400）
/// - 未知 `type` → `Ok(None)`（调用方记 debug 后忽略）
/// - 已知 `type` → 规范化为 [`DshEvent`]（detail 截断 200 字符，空 title/reason 视为缺失）
pub fn parse_event(body: &str) -> Result<Option<DshEvent>, String> {
    let RawEvent {
        r#type,
        session_id,
        title,
        reason,
        detail,
    } = serde_json::from_str(body).map_err(|e| format!("事件载荷不是合法 JSON 对象: {e}"))?;
    let title = title.filter(|t| !t.trim().is_empty());
    let reason = reason.filter(|r| !r.trim().is_empty());
    let detail = detail
        .map(|d| d.chars().take(200).collect::<String>())
        .filter(|d| !d.trim().is_empty());
    Ok(match r#type.as_str() {
        "task-started" => Some(DshEvent::TaskStarted { session_id, title }),
        "task-finished" => Some(DshEvent::TaskFinished {
            session_id,
            title,
            reason,
        }),
        "task-failed" => Some(DshEvent::TaskFailed {
            session_id,
            title,
            reason,
            detail,
        }),
        "task-interrupted" => Some(DshEvent::TaskInterrupted {
            session_id,
            title,
            reason,
        }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_started_full() {
        let ev = parse_event(
            r#"{"type":"task-started","session_id":"s1","title":"修复登录超时","extra":"未知字段"}"#,
        )
        .unwrap()
        .expect("已知类型应返回 Some");
        assert_eq!(
            ev,
            DshEvent::TaskStarted {
                session_id: "s1".to_string(),
                title: Some("修复登录超时".to_string()),
            }
        );
        assert_eq!(ev.kind(), "task-started");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.title(), Some("修复登录超时"));
    }

    #[test]
    fn test_parse_failed_truncates_detail() {
        let long = "x".repeat(300);
        let ev = parse_event(&format!(
            r#"{{"type":"task-failed","session_id":"s2","detail":"{long}"}}"#
        ))
        .unwrap()
        .unwrap();
        match ev {
            DshEvent::TaskFailed { detail, .. } => {
                assert_eq!(detail.as_deref().map(str::len), Some(200));
            }
            other => panic!("应为 TaskFailed: {other:?}"),
        }
    }

    #[test]
    fn test_parse_unknown_type_returns_none() {
        assert!(
            parse_event(r#"{"type":"todo-changed","session_id":"s"}"#)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_empty_title_treated_as_missing() {
        let ev = parse_event(r#"{"type":"task-started","session_id":"s","title":"  "}"#)
            .unwrap()
            .unwrap();
        assert_eq!(ev.title(), None);
    }

    #[test]
    fn test_parse_invalid_json_errs() {
        assert!(parse_event("不是json").is_err());
        assert!(parse_event(r#""裸字符串""#).is_err());
    }

    #[test]
    fn test_all_kinds() {
        for (body, kind) in [
            (
                r#"{"type":"task-started","session_id":"s"}"#,
                "task-started",
            ),
            (
                r#"{"type":"task-finished","session_id":"s"}"#,
                "task-finished",
            ),
            (r#"{"type":"task-failed","session_id":"s"}"#, "task-failed"),
            (
                r#"{"type":"task-interrupted","session_id":"s"}"#,
                "task-interrupted",
            ),
        ] {
            assert_eq!(parse_event(body).unwrap().unwrap().kind(), kind);
        }
    }
}
