use crate::{assert_todo_exists, db_err, parse_due, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand, SimplePluginCommand};
use nu_protocol::{
    Category, LabeledError, PipelineData, Signature, Span, SyntaxShape, Type, Value,
};

/// Struct for the 'todu desc` command
pub struct ToduDesc;

impl SimplePluginCommand for ToduDesc {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu desc"
    }
    fn description(&self) -> &str {
        "Set or update the description for a todo (pass \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu desc")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required("text", SyntaxShape::String, "Description text")
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
        let id: i64 = call.req(0)?;
        let text: String = call.req(1)?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            db.update_desc(id, proj, &text).map_err(db_err)?;
            Ok(Value::nothing(call.head))
        })
    }
}

/// Struct for the `todu tag` command
pub struct ToduTag;

impl PluginCommand for ToduTag {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu tag"
    }
    fn description(&self) -> &str {
        "Set or clear the tag for one or more todos (pass \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu tag")
            .required(
                "tag",
                SyntaxShape::String,
                "Tag name (without #), or \"\" to clear",
            )
            .rest("ids", SyntaxShape::Int, "Todu IDs (or pipe a list of IDs)")
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_types(vec![
                (Type::Nothing, Type::Nothing),
                (Type::List(Box::new(Type::Int)), Type::Nothing),
            ])
            .category(Category::Custom("todu".into()))
    }

    fn run(
        &self,
        plugin: &ToduPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let tag: String = call.req(0)?;
        let ids: Vec<(i64, Span)> = match input {
            PipelineData::Empty => {
                let ids: Vec<(i64, Span)> = call.positional[1..]
                    .iter()
                    .map(|v| {
                        v.as_int()
                            .map(|i| (i, v.span()))
                            .map_err(|e| LabeledError::new(e.to_string()))
                    })
                    .collect::<Result<_, _>>()?;
                if ids.is_empty() {
                    return Err(LabeledError::new(
                        "provide at least one ID argument or pipe a list of IDs",
                    ));
                }
                ids
            }
            _ => input
                .into_iter()
                .map(|v| {
                    v.as_int()
                        .map(|i| (i, call.head))
                        .map_err(|e| LabeledError::new(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };

        plugin.with_project(engine, call, |db, proj| {
            let tag_val = if tag.is_empty() {
                None
            } else {
                Some(tag.as_str())
            };
            for (id, span) in &ids {
                assert_todo_exists(db, *id, proj, *span)?;
                #[cfg(feature = "remote")]
                {
                    let row = db.get_todo(*id, proj).map_err(db_err)?;
                    if row.source != todu_db::ToduSource::Local {
                        return Err(LabeledError::new(format!(
                            "todo #{id} originates from {} — its tag is the issue identifier and cannot be changed",
                            row.source.label()
                        ))
                        .with_label("remote todo", *span));
                    }
                }
                db.update_tag(*id, proj, tag_val).map_err(db_err)?;
            }
            Ok(())
        })?;
        Ok(PipelineData::Empty)
    }
}

/// Struct for the `todu due` command
pub struct ToduDue;

impl SimplePluginCommand for ToduDue {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu due"
    }
    fn description(&self) -> &str {
        "Set or clear the due date for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu due")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required(
                "date",
                SyntaxShape::String,
                "Due date (YYYY-MM-DD, natural language, or \"none\"/\"\" to clear)",
            )
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
        let id: i64 = call.req(0)?;
        let date: String = call.req(1)?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            let due = if date.is_empty() || date.eq_ignore_ascii_case("none") {
                None
            } else {
                parse_due(&date)?
            };
            db.update_due(id, proj, due).map_err(db_err)?;
            Ok(Value::nothing(call.head))
        })
    }
}
