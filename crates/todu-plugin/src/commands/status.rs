use super::collect_ids;
use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};
use todu_db::ToduStatus;

fn apply_status(
    plugin: &ToduPlugin,
    engine: &EngineInterface,
    call: &EvaluatedCall,
    input: PipelineData,
    status: ToduStatus,
) -> Result<PipelineData, LabeledError> {
    let ids = collect_ids(call, input)?;
    plugin.with_project(engine, call, |db, proj| {
        for id in &ids {
            assert_todo_exists(db, *id, proj, call.head)?;
            db.set_todo_status(*id, proj, status).map_err(db_err)?;
        }
        Ok(PipelineData::Empty)
    })
}

macro_rules! status_cmd {
    ($name:ident, $cmd:expr, $desc:expr, $status:expr) => {
        #[doc = $desc]
        pub struct $name;

        impl PluginCommand for $name {
            type Plugin = ToduPlugin;

            fn name(&self) -> &str {
                $cmd
            }
            fn description(&self) -> &str {
                $desc
            }

            fn signature(&self) -> Signature {
                Signature::build($cmd)
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
                apply_status(plugin, engine, call, input, $status)
            }
        }
    };
}

status_cmd!(
    ToduDone,
    "todu done",
    "Mark a todo as done",
    ToduStatus::Done
);
status_cmd!(
    ToduStart,
    "todu start",
    "Mark a todo as in progress",
    ToduStatus::InProgress
);
status_cmd!(
    ToduStop,
    "todu stop",
    "Mark a todo as stopped",
    ToduStatus::Stopped
);
status_cmd!(ToduPause, "todu pause", "Pause a todo", ToduStatus::Paused);
status_cmd!(
    ToduReopen,
    "todu reopen",
    "Reset a todo back to pending",
    ToduStatus::Pending
);
