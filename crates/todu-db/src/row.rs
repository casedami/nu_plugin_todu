use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use nu_ansi_term::{Color, Style};
use nu_protocol::{Record, Span, Value};
use rusqlite::{Result as SqlResult, Row};

use super::{ToduPriority, ToduSource, ToduStatus};

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
    pub priority: Option<ToduPriority>,
    /// Task due date
    pub due: Option<NaiveDate>,
    /// Additional task description
    pub desc: Option<String>,
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
            priority: row
                .get::<_, Option<String>>(1)?
                .as_deref()
                .and_then(ToduPriority::from_str),
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

    /// Constructs a todu row for output
    pub fn render(&self, span: Span, long: bool) -> Value {
        let mut rec = Record::new();
        rec.push("id", Value::int(self.ptid, span));

        let due = self.due.and_then(|d| {
            Local
                .from_local_datetime(&d.and_hms_opt(23, 59, 59).unwrap())
                .single()
                .map(|dt| dt.fixed_offset())
        });

        let title = {
            let style = if !self.status.is_active() {
                Style::new().dimmed().strikethrough()
            } else if self.status == ToduStatus::Paused {
                Style::new().dimmed()
            } else if due.is_some_and(is_overdue) {
                Color::LightRed.bold().italic()
            } else {
                Style::new()
            };
            Value::string(style.paint(&self.title).to_string(), span)
        };
        rec.push("title", title);
        rec.push("status", Value::custom(Box::new(self.status), span));
        if let Some(priority) = self.priority {
            rec.push("priority", Value::custom(Box::new(priority), span));
        }

        if let Some(d) = due {
            rec.push("due", Value::date(d, span));
        }

        if let Some(ref t) = self.tag {
            rec.push("tag", Value::string(t.clone(), span));
        }

        if !self.subtasks.is_empty() {
            let subtasks_val = if long {
                Value::list(
                    self.subtasks
                        .iter()
                        .map(|s| s.render(span, false))
                        .collect(),
                    span,
                )
            } else {
                let total = self
                    .subtasks
                    .iter()
                    .filter(|s| s.status != ToduStatus::Stopped)
                    .count();
                let done = self
                    .subtasks
                    .iter()
                    .filter(|s| s.status == ToduStatus::Done)
                    .count();
                Value::string(format!("{done}/{total}"), span)
            };
            rec.push("subtasks", subtasks_val);
        }

        if let Some(ref desc) = self.desc {
            let desc_val = if long {
                Value::string(desc.clone(), span)
            } else {
                Value::string(Style::new().dimmed().paint(TRUNCATED).to_string(), span)
            };
            rec.push("desc", desc_val);
        }

        if long {
            rec.push("source", Value::string(self.source.label(), span));
            if let Some(parent) = self.pptid {
                rec.push("parent", Value::int(parent, span));
            }
            rec.push("created", Value::date(self.created.fixed_offset(), span));
        }

        Value::record(rec, span)
    }
}

fn is_overdue(date: DateTime<FixedOffset>) -> bool {
    date < Local::now().fixed_offset()
}
