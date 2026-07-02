use nu_ansi_term::{Color, Style};
use nu_protocol::{ast::Operator, CustomValue, ShellError, Span, Value};
use rusqlite::types::{ToSql, ToSqlOutput};

use super::compare_ordering;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Priority level of a todo
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToduPriority {
    /// Low priority @!
    Low,
    /// Medium priority @!!
    Medium,
    /// High priority @!!!
    High,
}

impl ToduPriority {
    /// Parses a priority label string. Returns `None` for unrecognised values.
    pub fn from_input(s: &str) -> Option<Self> {
        match s {
            "high" => Some(Self::High),
            "medium" => Some(Self::Medium),
            "low" => Some(Self::Low),
            _ => None,
        }
    }

    /// Returns the canonical string label used for database storage and display
    pub fn label(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    fn styled(&self) -> Style {
        match self {
            Self::High => Color::LightRed.bold(),
            Self::Medium => Color::LightYellow.normal(),
            Self::Low => Color::LightBlue.normal(),
        }
    }

    /// Converts a nu `Value` into a `ToduPriority` for comparison operations.
    ///
    /// This allows things like `todo | where priority > low`.
    fn coerce(other: &Value) -> Option<Self> {
        match other {
            Value::String { val, .. } => Self::from_input(val),
            _ => None,
        }
    }
}

#[typetag::serde]
impl CustomValue for ToduPriority {
    fn type_name(&self) -> String {
        "priority".into()
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
                unsupported: nu_protocol::Type::Custom("priority".into()),
                op_span: op,
                unsupported_span: lhs_span,
                help: None,
            }),
        }
    }
}

impl ToSql for ToduPriority {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.label()))
    }
}
