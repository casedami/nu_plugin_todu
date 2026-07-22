use super::collect_value_and_ids;
use crate::{assert_todo_exists, db_err, parse_due, ToduPlugin};

/// Struct for the `todu title` command
pub struct ToduTitle;

impl PluginCommand for ToduTitle {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu title"
    }
    fn description(&self) -> &str {
        "Edit the title of a todo"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu title")
            .required("text", SyntaxShape::String, "New title text")
            .rest("ids", SyntaxShape::Int, "Todu ID(s) (or pipe ids in)")
            .switch(
                "append",
                "Append text to existing title instead of replacing",
                Some('a'),
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
        let (text, ids) = collect_value_and_ids(call, input, "title")?;
        let append = call.has_flag("append")?;
        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;
                let title = if append {
                    let existing = db.get_todo(*id, proj).map_err(db_err)?;
                    format!("{} {text}", existing.title)
                } else {
                    text.clone()
                };
                db.update_title(*id, proj, &title).map_err(db_err)?;
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
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

/// Struct for the 'todu desc` command
pub struct ToduDesc;

impl PluginCommand for ToduDesc {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu desc"
    }
    fn description(&self) -> &str {
        "Set or update the description for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu desc")
            .required("text", SyntaxShape::String, "Description text")
            .rest("ids", SyntaxShape::Int, "Todu ID(s) (or pipe ids in)")
            .switch(
                "append",
                "Append text to existing description instead of replacing",
                Some('a'),
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
        let (text, ids) = collect_value_and_ids(call, input, "desc")?;
        let append = call.has_flag("append")?;
        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;
                let desc = if text.is_empty() || text.eq_ignore_ascii_case("none") {
                    None
                } else if append {
                    let existing = db.get_todo(*id, proj).map_err(db_err)?;
                    let combined = match existing.desc {
                        Some(prev) => format!("{prev}\n{text}"),
                        None => text.clone(),
                    };
                    Some(combined)
                } else {
                    Some(text.clone())
                };
                db.update_desc(*id, proj, desc.as_deref()).map_err(db_err)?;
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

/// Struct for the `todu tag` command
pub struct ToduTag;

impl PluginCommand for ToduTag {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu tag"
    }
    fn description(&self) -> &str {
        "Set or clear the tag for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu tag")
            .required(
                "tag",
                SyntaxShape::String,
                "Tag name (without #), or \"\" to clear",
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
        let (tag, ids) = collect_value_and_ids(call, input, "tag")?;
        let tag_val = if tag.is_empty() || tag.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(tag)
        };
        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;
                #[cfg(feature = "remote")]
                {
                    let row = db.get_todo(*id, proj).map_err(db_err)?;
                    if row.source != todu_db::ToduSource::Local {
                        return Err(LabeledError::new(format!(
                            "todo #{id} originates from {} — its tag is the issue identifier and cannot be changed",
                            row.source.label()
                        ))
                        .with_label("remote todo", head));
                    }
                }
                db.update_tag(*id, proj, tag_val.as_deref()).map_err(db_err)?;
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

/// Struct for the `todu due` command
pub struct ToduDue;

impl PluginCommand for ToduDue {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu due"
    }
    fn description(&self) -> &str {
        "Set or clear the due date for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu due")
            .required(
                "date",
                SyntaxShape::String,
                "Due date (YYYY-MM-DD, natural language, or \"none\"/\"\" to clear)",
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
        let (date, ids) = collect_value_and_ids(call, input, "due")?;
        let due = if date.is_empty() || date.eq_ignore_ascii_case("none") {
            None
        } else {
            parse_due(&date)?
        };
        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for id in &ids {
                assert_todo_exists(db, *id, proj, head)?;
                db.update_due(*id, proj, due).map_err(db_err)?;
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
