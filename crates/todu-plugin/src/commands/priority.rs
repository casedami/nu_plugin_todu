use super::collect_value_and_ids;
use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};
use todu_db::ToduPriority;

/// Struct for the `todu priority` command
pub struct ToduPriorityCmd;

impl PluginCommand for ToduPriorityCmd {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu priority"
    }
    fn description(&self) -> &str {
        "Set or clear the priority for a todo (low, medium, high, or \"none\"/\"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu priority")
            .required(
                "level",
                SyntaxShape::String,
                "Priority level: low, medium, high, or \"none\"/\"\" to clear",
            )
            .rest("ids", SyntaxShape::Int, "Todu ID(s) (or pipe ids in)")
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_type(Type::Nothing, Type::Any)
            .input_output_type(Type::Int, Type::Any)
            .input_output_type(Type::List(Box::new(Type::Int)), Type::Any)
            .category(Category::Custom("todu".into()))
    }

    fn run(
        &self,
        plugin: &ToduPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let (level, ids) = collect_value_and_ids(call, input, "priority")?;
        let priority = if level.is_empty() || level.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(
                ToduPriority::from_input(&level.to_lowercase()).ok_or_else(|| {
                    LabeledError::new(format!(
                        "unknown priority \"{level}\" — expected low, medium, or high"
                    ))
                    .with_label("invalid priority", call.positional[0].span())
                })?,
            )
        };
        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;
                db.update_priority(*id, proj, priority).map_err(db_err)?;
                let row = db.get_todo_tree(*id, proj).map_err(db_err)?;
                rendered.push(row.render_long(head));
            }
            let value = if rendered.len() == 1 {
                rendered.remove(0)
            } else {
                Value::list(rendered, head)
            };
            Ok(PipelineData::Value(value, None))
        })
    }
}
