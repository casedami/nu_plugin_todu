use crate::{db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, Type, Value};

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
        "List all todos for the current directory"
    }

    fn extra_description(&self) -> &str {
        "Subcommands: add, start, done, stop, pause, reopen, due, desc, clear"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu")
            .switch(
                "long",
                "Show full view including created date and full desc",
                Some('l'),
            )
            .switch(
                "global",
                "Use home directory as project instead of git root",
                Some('g'),
            )
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
        let long: bool = call.has_flag("long")?;
        plugin.with_project(engine, call, |db, proj| {
            let rows = db.get_live_todos(proj).map_err(db_err)?;
            let span = call.head;
            let result = if rows.is_empty() {
                let idx = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as usize)
                    .unwrap_or(0)
                    % EMPTY_MSGS.len();
                Value::string(EMPTY_MSGS[idx], span)
            } else {
                Value::list(rows.iter().map(|r| r.render(span, long)).collect(), span)
            };
            Ok(result)
        })
    }
}
