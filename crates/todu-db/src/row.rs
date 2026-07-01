use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use nu_ansi_term::{Color, Style};
use nu_protocol::{Record, Span, Value};
use rusqlite::{Result as SqlResult, Row};

use super::{ToduPriority, ToduSource, ToduStatus};

const EMPTY: &str = "---";
const TRUNCATED: &str = "...";

/// A single todo row as returned by the database
pub struct ToduRow {
    /// Project-scoped unique ID (monotonically increasing per project).
    pub ptid: i64,
    /// Task title
    pub title: String,
    /// Task status
    pub status: ToduStatus,
    /// Task priority level
    pub priority: ToduPriority,
    /// Task due date
    pub due: Option<NaiveDate>,
    /// Additional task description
    pub desc: String,
    /// Task creation date
    pub created: DateTime<Utc>,
    /// `ptid` of the parent task, or `None` for root-level todos.
    pub pptid: Option<i64>,
    /// Optional tag associated with the task
    pub tag: Option<String>,
    /// Source of the task (local/remote)
    pub source: ToduSource,
    /// Subtasks (if any)
    pub subtasks: Vec<ToduRow>,
}

impl ToduRow {
    pub(super) const COLS: &'static str =
        "ptid, priority, status, title, due, desc, pptid, created, tag, source";

    /// Deserializes a SQLite row into a `ToduRow`
    pub(super) fn from_sql(row: &Row) -> SqlResult<Self> {
        Ok(Self {
            ptid: row.get(0)?,
            priority: row.get(1)?,
            status: row.get(2)?,
            title: row.get(3)?,
            due: row.get(4)?,
            desc: row.get(5)?,
            pptid: row.get(6)?,
            created: DateTime::<Utc>::from_timestamp(row.get::<_, i64>(7)?, 0).unwrap_or_default(),
            tag: row.get(8)?,
            source: ToduSource::from_str(&row.get::<_, String>(9)?),
            subtasks: Vec::new(),
        })
    }

    /// Renders the row as a nu `Value::record`. Pass `long = true` for the full view including
    /// description, source, and created date; `false` for the compact list view.
    pub fn render(&self, span: Span, long: bool) -> Value {
        let mut rec = Record::new();

        rec.push("id", Value::int(self.ptid, span));
        rec.push("title", render_title(&self.title, &self.status, span));
        rec.push("status", render_status(&self.status, span));
        rec.push("priority", render_priority(&self.priority, span));
        rec.push(
            "desc",
            if self.desc.is_empty() {
                render_empty(span)
            } else if long {
                Value::string(self.desc.clone(), span)
            } else {
                render_truncated(span)
            },
        );
        rec.push("due", render_due(&self.due, span));
        rec.push("subtasks", self.render_subtasks(span, long));
        rec.push(
            "tag",
            match &self.tag {
                Some(t) => Value::string(t.clone(), span),
                None => render_empty(span),
            },
        );
        rec.push("source", Value::string(self.source.short_label(), span));

        if long {
            rec.push("created", Value::date(self.created.fixed_offset(), span));
        }

        Value::record(rec, span)
    }

    fn render_subtasks(&self, span: Span, long: bool) -> Value {
        if self.subtasks.is_empty() {
            render_empty(span)
        } else if long {
            Value::list(
                self.subtasks.iter().map(|s| s.render(span, long)).collect(),
                span,
            )
        } else {
            let active = self
                .subtasks
                .iter()
                .filter(|s| s.status.is_active())
                .count();
            Value::int(active as i64, span)
        }
    }
}

fn render_empty(span: Span) -> Value {
    Value::string(Style::new().dimmed().paint(EMPTY).to_string(), span)
}

fn render_truncated(span: Span) -> Value {
    Value::string(Style::new().dimmed().paint(TRUNCATED).to_string(), span)
}

fn render_title(title: &str, status: &ToduStatus, span: Span) -> Value {
    let styled = match status {
        ToduStatus::Done | ToduStatus::Paused | ToduStatus::Stopped => {
            Style::new().dimmed().paint(title).to_string()
        }
        _ => title.to_string(),
    };
    Value::string(styled, span)
}

fn render_due(date: &Option<NaiveDate>, span: Span) -> Value {
    match date {
        None => render_empty(span),
        Some(date) if is_overdue(*date) => Value::string(
            Color::LightRed
                .underline()
                .paint(date.format("%Y-%m-%d").to_string())
                .to_string(),
            span,
        ),
        Some(date) => Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .map(|dt| Value::date(dt.fixed_offset(), span))
            .unwrap_or_else(|| Value::string(date.format("%Y-%m-%d").to_string(), span)),
    }
}

fn is_overdue(date: NaiveDate) -> bool {
    date < Local::now().date_naive()
}

fn render_priority(priority: &ToduPriority, span: Span) -> Value {
    Value::custom(Box::new(*priority), span)
}

fn render_status(status: &ToduStatus, span: Span) -> Value {
    Value::custom(Box::new(*status), span)
}
