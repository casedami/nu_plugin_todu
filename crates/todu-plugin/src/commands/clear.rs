use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

/// Struct for the `todu clear` command
pub struct ToduClear;

impl PluginCommand for ToduClear {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu clear"
    }
    fn description(&self) -> &str {
        "Archive a todo by id (or piped ids), or every todo with --all. Done/stopped todos are \
         archived automatically as they happen. With --hard, permanently purge instead of archiving"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu clear")
            .optional("id", SyntaxShape::Int, "Todu ID (or pipe a list of IDs)")
            .switch("all", "Act on every todo in the project", Some('a'))
            .switch("hard", "Permanently delete instead of archiving", Some('H'))
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_type(Type::Nothing, Type::Nothing)
            .input_output_type(Type::Int, Type::Nothing)
            .input_output_type(Type::List(Box::new(Type::Int)), Type::Nothing)
            .category(Category::Custom("todu".into()))
    }

    fn run(
        &self,
        plugin: &ToduPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let all: bool = call.has_flag("all")?;
        let hard: bool = call.has_flag("hard")?;
        let ids: Vec<i64> = match input {
            PipelineData::Empty => call.opt(0)?.into_iter().collect(),
            _ => input
                .into_iter()
                .map(|v| v.as_int().map_err(|e| LabeledError::new(e.to_string())))
                .collect::<Result<_, _>>()?,
        };

        if ids.is_empty() != all {
            return Err(LabeledError::new(if all {
                "todu clear: pass either an id/piped ids or --all, not both"
            } else {
                "todu clear requires an id (or piped ids) or --all"
            }));
        }

        plugin.with_project(engine, call, |db, proj| {
            let count = if ids.is_empty() {
                if hard {
                    db.purge_deleted(proj).map_err(db_err)?
                } else {
                    db.clear_all(proj).map_err(db_err)?
                }
            } else {
                let mut n = 0;
                for id in &ids {
                    assert_todo_exists(db, *id, proj, call.head)?;
                    n += if hard {
                        db.purge_todo(*id, proj).map_err(db_err)?
                    } else {
                        db.delete_todo(*id, proj).map_err(db_err)?
                    };
                }
                n
            };
            let label = if hard { "Purged" } else { "Archived" };
            Ok(PipelineData::Value(
                Value::string(format!("{label} {count} todo(s)"), call.head),
                None,
            ))
        })
    }
}
