use nu_protocol::{ast::Operator, CustomValue, ShellError, Span, Value};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use super::compare_ordering;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Priority level of a todo
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToduPriority {
    /// High priority @!!!
    High = 0,
    /// Medium priority @!!
    Medium = 1,
    /// Low priority @!
    Low = 2,
    /// No priority
    Unset = 3,
}

impl From<u8> for ToduPriority {
    fn from(n: u8) -> Self {
        match n {
            0 => Self::High,
            1 => Self::Medium,
            2 => Self::Low,
            _ => Self::Unset,
        }
    }
}

impl ToduPriority {
    /// Parses a priority label string. Returns `None` for unrecognised values.
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            s if s == "---" || s == "none" => Self::Unset,
            _ => return None,
        })
    }

    /// Returns the canonical string label used for database storage and display
    pub fn label(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unset => "---",
        }
    }

    /// Converts a nu `Value` into a `ToduPriority` for comparison operations.
    ///
    /// This allows things like `todo | where priority > low`.
    fn coerce(other: &Value) -> Option<Self> {
        match other {
            Value::String { val, .. } => Self::from_str(val),
            Value::Int { val, .. } => Some(Self::from(*val as u8)),
            Value::Custom { val, .. } => val.as_any().downcast_ref::<ToduPriority>().cloned(),
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
        Ok(Value::string(self.label().to_string(), span))
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

impl FromSql for ToduPriority {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()
            .map(|s| ToduPriority::from_str(s).unwrap_or(ToduPriority::Unset))
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}
