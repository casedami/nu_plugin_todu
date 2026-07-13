use crate::{db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, Type, Value};
use todu_db::ToduRow;

const EMPTY_MSGS: &[&str] = &[
    "No todos — add one with: todu add <task>",
    "No todos — kick back and relax",
    "No todos — you're all caught up",
    "No todos — enjoy the silence",
    "No todos — the slate is clean",
    "No todos — nothing to do here",
];

/// Struct for the `todu` command
pub struct ToduList;

impl SimplePluginCommand for ToduList {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu"
    }

    fn description(&self) -> &str {
        "List all todos for the current project"
    }

    fn extra_description(&self) -> &str {
        "Subcommands: add, branch, clear, desc, done, due, get, move, pause, priority, pull, remote, reopen, rm, start, stop, tag, title"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu")
            .switch(
                "global",
                "Use home directory as project instead of git root",
                Some('g'),
            )
            .switch("overdue", "Show only overdue tasks", Some('o'))
            .input_output_type(Type::Nothing, Type::Any)
            .category(Category::Custom("todu".into()))
    }

    fn run(
        &self,
        plugin: &ToduPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let overdue: bool = call.has_flag("overdue")?;
        plugin.with_project(engine, call, |db, proj| {
            let rows = db.get_live_todos(proj).map_err(db_err)?;
            let span = call.head;
            let result = if overdue {
                let mut flat = Vec::new();
                collect_overdue(&rows, &mut flat);
                if flat.is_empty() {
                    Value::string("No overdue todos", span)
                } else {
                    Value::list(flat.iter().map(|r| r.render_short(span)).collect(), span)
                }
            } else if rows.is_empty() {
                let idx = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as usize)
                    .unwrap_or(0)
                    % EMPTY_MSGS.len();
                Value::string(EMPTY_MSGS[idx], span)
            } else if rows.len() == 1 {
                rows[0].render_long(span)
            } else {
                Value::list(rows.iter().map(|r| r.render_short(span)).collect(), span)
            };
            Ok(result)
        })
    }
}

fn collect_overdue<'a>(rows: &'a [ToduRow], out: &mut Vec<&'a ToduRow>) {
    for row in rows {
        if row.is_overdue() {
            out.push(row);
        }
        collect_overdue(&row.subtasks, out);
    }
}
