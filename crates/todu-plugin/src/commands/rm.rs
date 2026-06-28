use super::collect_ids;
use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};

/// Struct for the `todu rm` command
pub struct ToduRm;

impl PluginCommand for ToduRm {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu rm"
    }
    fn description(&self) -> &str {
        "Remove a todo (soft-delete)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu rm")
            .optional("id", SyntaxShape::Int, "Todu ID (or pipe a list of IDs)")
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
        let ids = collect_ids(call, input)?;
        plugin.with_project(engine, call, |db, proj| {
            for id in &ids {
                assert_todo_exists(db, *id, proj, call.head)?;
                db.delete_todo(*id, proj).map_err(db_err)?;
            }
            Ok(PipelineData::Empty)
        })
    }
}
