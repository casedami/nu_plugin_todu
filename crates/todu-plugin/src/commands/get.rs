use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Type, Value};

/// Struct for the `todu get` command
pub struct ToduGet;

impl SimplePluginCommand for ToduGet {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu get"
    }

    fn description(&self) -> &str {
        "Get a todo by its ID"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu get")
            .required("id", SyntaxShape::Int, "Todu ID")
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
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            let row = db.get_todo_tree(id, proj).map_err(db_err)?;
            Ok(row.render_long(call.head))
        })
    }
}
