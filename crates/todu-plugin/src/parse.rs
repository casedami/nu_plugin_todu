use chrono::{Local, NaiveDate};
use nu_protocol::LabeledError;
use todu_db::{ParsedTodu, ToduPriority, ToduSource};

/// Parses a date string in `YYYY-MM-DD` or natural-language form (e.g. `"friday"`, `"next week"`)
pub fn parse_due(raw: &str) -> Result<Option<NaiveDate>, LabeledError> {
    let now = Local::now();
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .or_else(|_| {
            chrono_english::parse_date_string(raw, now, chrono_english::Dialect::Us)
                .map(|dt| dt.date_naive())
                .map_err(|_| ())
        })
        .or_else(|_| {
            if raw.contains(|c: char| c.is_ascii_digit()) {
                return Err(());
            }
            let spaced = raw.replace('-', " ");
            chrono_english::parse_date_string(&spaced, now, chrono_english::Dialect::Us)
                .map(|dt| dt.date_naive())
                .map_err(|_| ())
        })
        .map(Some)
        .map_err(|_| {
            LabeledError::new(format!(
                "Invalid date '{raw}' — use YYYY-MM-DD or a natural date like 'friday', 'next-friday'"
            ))
        })
}

/// Parses the inline task string format into a [`ParsedTodu`].
///
/// Format: `[tokens...] task text [@due-date] [// description]`
///
/// Recognised inline tokens (may appear anywhere in the task portion):
/// - `#tag`   — categorisation label
/// - `^N`     — numeric parent task UID
/// - `!` / `!!` / `!!!` — low / medium / high priority
/// - `@date`  — due date (YYYY-MM-DD or natural language)
pub fn parse_inline(input: &str) -> Result<ParsedTodu, LabeledError> {
    let (task_part, desc_part) = input.split_once(" // ").unwrap_or((input, ""));

    let mut tag: Option<String> = None;
    let mut pptid: Option<i64> = None;
    let mut priority: Option<ToduPriority> = None;
    let mut due_result: Option<Result<Option<NaiveDate>, LabeledError>> = None;

    let title: String = task_part
        .trim_start_matches(char::is_whitespace)
        .split_whitespace()
        .filter_map(|token| {
            // @date — due date
            if due_result.is_none() {
                if let Some(date_str) = token.strip_prefix('@') {
                    if !date_str.is_empty() {
                        due_result = Some(parse_due(date_str));
                        return None;
                    }
                }
            }
            // #tag
            if tag.is_none() {
                if let Some(t) = token.strip_prefix('#') {
                    if !t.is_empty() {
                        tag = Some(t.to_string());
                        return None;
                    }
                }
            }
            // ^parent
            if pptid.is_none() {
                if let Some(p) = token.strip_prefix('^') {
                    if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(id) = p.parse::<i64>() {
                            pptid = Some(id);
                            return None;
                        }
                    }
                }
            }
            // ! / !! / !!! — priority; may be standalone (`!!!`) or a prefix (`!!!task`)
            if priority.is_none() {
                let bang_count = token.chars().take_while(|c| *c == '!').count();
                if bang_count > 0 {
                    let n = bang_count.min(3);
                    priority = Some(match n {
                        1 => ToduPriority::Low,
                        2 => ToduPriority::Medium,
                        _ => ToduPriority::High,
                    });
                    let rest = &token[bang_count..];
                    return if rest.is_empty() { None } else { Some(rest) };
                }
            }
            Some(token)
        })
        .collect::<Vec<_>>()
        .join(" ");

    let due = match due_result {
        Some(r) => r?,
        None => None,
    };

    Ok(ParsedTodu {
        title,
        priority,
        due,
        desc: if desc_part.is_empty() { None } else { Some(desc_part.to_string()) },
        pptid,
        tag,
        source: ToduSource::Local,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_due, parse_inline, ToduPriority};
    use chrono::NaiveDate;

    #[test]
    fn parse_due_iso_date() {
        let result = parse_due("2026-01-15").unwrap();
        assert_eq!(result, Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()));
    }

    #[rstest::rstest]
    #[case("not-a-date")]
    #[case("")]
    #[case("2026-13-01")]
    #[case("2026-00-01")]
    fn parse_due_invalid_returns_err(#[case] input: &str) {
        assert!(parse_due(input).is_err());
    }

    #[rstest::rstest]
    #[case("tomorrow")]
    #[case("next monday")]
    #[case("friday")]
    #[case("next-monday")]
    #[case("next-friday")]
    fn parse_due_natural_language_returns_some(#[case] input: &str) {
        assert!(parse_due(input).unwrap().is_some());
    }

    #[test]
    fn basic_task() {
        let item = parse_inline("buy milk").unwrap();
        assert_eq!(item.title, "buy milk");
        assert_eq!(item.priority, None);
        assert_eq!(item.desc, None);
        assert_eq!(item.due, None);
        assert!(item.tag.is_none());
        assert!(item.pptid.is_none());
    }

    #[rstest::rstest]
    #[case("task !", Some(ToduPriority::Low))]
    #[case("task !!", Some(ToduPriority::Medium))]
    #[case("task !!!", Some(ToduPriority::High))]
    #[case("task !!!!!", Some(ToduPriority::High))]
    fn priority(#[case] input: &str, #[case] expected: Option<ToduPriority>) {
        assert_eq!(parse_inline(input).unwrap().priority, expected);
    }

    #[rstest::rstest]
    #[case("task !!", "task")]
    #[case("task #work", "task")]
    #[case("task ^3", "task")]
    fn token_removed_from_task(#[case] input: &str, #[case] expected_task: &str) {
        assert_eq!(parse_inline(input).unwrap().title, expected_task);
    }

    #[rstest::rstest]
    #[case("task #work", Some("work"))]
    #[case("task #", None)]
    fn tag(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(parse_inline(input).unwrap().tag.as_deref(), expected);
    }

    #[rstest::rstest]
    #[case("task ^3", Some(3))]
    #[case("task ^", None)]
    fn parent(#[case] input: &str, #[case] expected: Option<i64>) {
        assert_eq!(parse_inline(input).unwrap().pptid, expected);
    }

    #[test]
    fn empty_tag_keeps_token_in_task() {
        assert_eq!(parse_inline("task #").unwrap().title, "task #");
    }

    #[test]
    fn empty_parent_keeps_token_in_task() {
        assert_eq!(parse_inline("task ^").unwrap().title, "task ^");
    }

    #[test]
    fn inline_desc() {
        let item = parse_inline("task // my description").unwrap();
        assert_eq!(item.title, "task");
        assert_eq!(item.desc.as_deref(), Some("my description"));
    }

    #[test]
    fn due_in_task() {
        let item = parse_inline("task @2026-07-01").unwrap();
        assert_eq!(item.title, "task");
        assert_eq!(item.due, parse_due("2026-07-01").unwrap());
    }

    #[test]
    fn due_natural_language() {
        let item = parse_inline("task @friday").unwrap();
        assert_eq!(item.title, "task");
        assert!(item.due.is_some());
    }

    #[test]
    fn at_in_desc_is_literal() {
        let item = parse_inline("task // description @2026-07-01").unwrap();
        assert_eq!(item.title, "task");
        assert_eq!(item.desc.as_deref(), Some("description @2026-07-01"));
        assert_eq!(item.due, None);
    }

    #[test]
    fn combined_tokens() {
        let item = parse_inline("task !! #work ^2 // some desc").unwrap();
        assert_eq!(item.title, "task");
        assert_eq!(item.priority, Some(ToduPriority::Medium));
        assert_eq!(item.tag.as_deref(), Some("work"));
        assert_eq!(item.pptid, Some(2));
        assert_eq!(item.desc.as_deref(), Some("some desc"));
    }

    #[test]
    fn invalid_date_returns_error() {
        assert!(parse_inline("task @not-a-date").is_err());
    }

    #[test]
    fn priority_token_with_due() {
        let item = parse_inline("task !!! @tomorrow").unwrap();
        assert_eq!(item.title, "task");
        assert_eq!(item.priority, Some(ToduPriority::High));
        assert!(item.due.is_some());
    }

    #[test]
    fn priority_prefix_attached_to_word() {
        let item = parse_inline("!!!test @tomorrow").unwrap();
        assert_eq!(item.title, "test");
        assert_eq!(item.priority, Some(ToduPriority::High));
        assert!(item.due.is_some());
    }
}
