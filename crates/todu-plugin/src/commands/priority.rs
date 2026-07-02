use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Type, Value};
use todu_db::ToduPriority;

/// Struct for the `todu priority` command
pub struct ToduPriorityCmd;

impl SimplePluginCommand for ToduPriorityCmd {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu priority"
    }
    fn description(&self) -> &str {
        "Set or clear the priority for a todo (low, medium, high, or \"none\"/\"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu priority")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required(
                "level",
                SyntaxShape::String,
                "Priority level: low, medium, high, or \"none\"/\"\" to clear",
            )
            .switch("global", "Use home directory as project", Some('g'))
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
        let id: i64 = call.req(0)?;
        let level: String = call.req(1)?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            let priority = if level.is_empty() || level.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(
                    ToduPriority::from_input(&level.to_lowercase()).ok_or_else(|| {
                        LabeledError::new(format!(
                            "unknown priority \"{level}\" — expected low, medium, or high"
                        ))
                        .with_label("invalid priority", call.positional[1].span())
                    })?,
                )
            };
            db.update_priority(id, proj, priority).map_err(db_err)?;
            let row = db.get_todo_tree(id, proj).map_err(db_err)?;
            Ok(row.render_long(call.head))
        })
    }
}
