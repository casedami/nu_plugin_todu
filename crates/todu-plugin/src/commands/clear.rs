use crate::db_err;
use crate::ToduPlugin;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, Type, Value};

/// Struct for the `todu clear` command
pub struct ToduClear;

impl SimplePluginCommand for ToduClear {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu clear"
    }
    fn description(&self) -> &str {
        "Archive done/stopped todos; with --all, archive every todo; with --hard, permanently delete archived todos"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu clear")
            .switch("all", "Archive all todos, not just done/stopped", Some('a'))
            .switch("hard", "Permanently delete all archived todos", Some('H'))
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_type(Type::Nothing, Type::Nothing)
            .category(Category::Custom("todu".into()))
    }

    fn run(
        &self,
        plugin: &ToduPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let all: bool = call.has_flag("all")?;
        let hard: bool = call.has_flag("hard")?;
        plugin.with_project(engine, call, |db, proj| {
            let (count, label) = if hard {
                (db.purge_deleted(proj).map_err(db_err)?, "Purged")
            } else if all {
                (db.clear_all(proj).map_err(db_err)?, "Archived")
            } else {
                (db.clear_done(proj).map_err(db_err)?, "Archived")
            };
            Ok(Value::string(format!("{label} {count} todo(s)"), call.head))
        })
    }
}
