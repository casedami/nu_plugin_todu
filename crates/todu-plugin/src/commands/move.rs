use super::collect_value_and_ids;
use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

/// Struct for the `todu move` command
pub struct ToduMove;

impl PluginCommand for ToduMove {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu move"
    }

    fn description(&self) -> &str {
        "Reparent a todo (pass \"none\" or 0 to make it a root-level task)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu move")
            .required(
                "parent",
                SyntaxShape::String,
                "New parent ID, or \"none\"/0 to unparent",
            )
            .rest(
                "ids",
                SyntaxShape::Int,
                "Todu ID(s) to reparent (or pipe ids in)",
            )
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
        let (parent_str, ids) = collect_value_and_ids(call, input, "move")?;

        let new_parent: Option<i64> = if parent_str.is_empty()
            || parent_str.eq_ignore_ascii_case("none")
            || parent_str == "0"
        {
            None
        } else {
            let n: i64 = parent_str.parse().map_err(|_| {
                LabeledError::new(format!(
                    "invalid parent \"{parent_str}\" — expected an ID, 0, or \"none\""
                ))
                .with_label("invalid parent", call.positional[0].span())
            })?;
            Some(n)
        };

        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;

                if let Some(parent_id) = new_parent {
                    if parent_id == *id {
                        return Err(LabeledError::new("a todo cannot be its own parent")
                            .with_label("same as id", head));
                    }
                    assert_todo_exists(db, parent_id, proj, head)?;
                    if db.is_ancestor_of(*id, parent_id, proj).map_err(db_err)? {
                        return Err(LabeledError::new(format!(
                            "todo #{parent_id} is a descendant of #{id} — moving would create a cycle"
                        ))
                        .with_label("would create cycle", head));
                    }
                }

                db.update_parent(*id, proj, new_parent).map_err(db_err)?;
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
