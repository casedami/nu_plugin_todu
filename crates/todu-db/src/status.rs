use nu_ansi_term::{Color, Style};
use nu_protocol::{ast::Operator, CustomValue, ShellError, Span, Value};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use super::compare_ordering;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Lifecycle state of a todo. Ordered so that active states sort before terminal ones
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToduStatus {
    /// Task has been started
    InProgress,
    /// Task has not been started yet
    Pending,
    /// Parent task with all children tasks marked as `Done`
    InReview,
    /// Task has been paused and is no longer in progress
    Paused,
    /// Task is no longer being pursued
    Stopped,
    /// Task has been finished
    Done,
}

impl ToduStatus {
    /// Parses a status string. Returns `None` for unrecognised values.
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "in-progress" => Self::InProgress,
            "in-review" => Self::InReview,
            "pending" => Self::Pending,
            "paused" => Self::Paused,
            "stopped" => Self::Stopped,
            "done" => Self::Done,
            _ => return None,
        })
    }

    /// Returns the string label used for database storage and display
    pub fn label(&self) -> &str {
        match self {
            Self::InProgress => "in-progress",
            Self::InReview => "in-review",
            Self::Pending => "pending",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Done => "done",
        }
    }

    fn styled(&self) -> Style {
        match self {
            Self::InProgress => Color::LightBlue.italic(),
            Self::InReview => Color::Green.underline(),
            Self::Pending => Color::LightBlue.normal(),
            Self::Paused => Color::LightGray.dimmed(),
            Self::Stopped => Color::LightGray.strikethrough(),
            Self::Done => Color::Green.dimmed().strikethrough(),
        }
    }

    /// Returns `true` for statuses that are not terminal (`Done` or `Stopped`)
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Done | Self::Stopped)
    }

    /// Converts a nu `Value` into a `ToduStatus` for comparison operations
    fn coerce(other: &Value) -> Option<Self> {
        match other {
            Value::String { val, .. } => Self::from_str(val),
            _ => None,
        }
    }
}

#[typetag::serde]
impl CustomValue for ToduStatus {
    fn type_name(&self) -> String {
        "status".into()
    }

    fn to_base_value(&self, span: Span) -> Result<Value, ShellError> {
        Ok(Value::string(
            self.styled().paint(self.label()).to_string(),
            span,
        ))
    }

    fn clone_value(&self, span: Span) -> Value {
        Value::custom(Box::new(*self), span)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        Self::coerce(other).map(|o| self.cmp(&o))
    }

    fn operation(
        &self,
        lhs_span: Span,
        operator: Operator,
        op: Span,
        right: &Value,
    ) -> Result<Value, ShellError> {
        let Some(rhs) = Self::coerce(right) else {
            return Err(ShellError::OperatorUnsupportedType {
                op: operator,
                unsupported: right.get_type(),
                op_span: op,
                unsupported_span: right.span(),
                help: None,
            });
        };
        match compare_ordering(self.cmp(&rhs), operator) {
            Some(b) => Ok(Value::bool(b, lhs_span)),
            None => Err(ShellError::OperatorUnsupportedType {
                op: operator,
                unsupported: nu_protocol::Type::Custom("status".into()),
                op_span: op,
                unsupported_span: lhs_span,
                help: None,
            }),
        }
    }
}

impl ToSql for ToduStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.label()))
    }
}

impl FromSql for ToduStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()
            .map(|s| ToduStatus::from_str(s).unwrap_or(ToduStatus::Pending))
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}
