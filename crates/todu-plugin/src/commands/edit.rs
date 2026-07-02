use crate::{assert_todo_exists, db_err, parse_due, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Type, Value};

/// Struct for the 'todu desc` command
pub struct ToduDesc;

impl SimplePluginCommand for ToduDesc {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu desc"
    }
    fn description(&self) -> &str {
        "Set or update the description for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu desc")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required("text", SyntaxShape::String, "Description text")
            .switch(
                "append",
                "Append text to existing description instead of replacing",
                Some('a'),
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
        let text: String = call.req(1)?;
        let append = call.has_flag("append")?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            let desc = if text.is_empty() || text.eq_ignore_ascii_case("none") {
                None
            } else if append {
                let existing = db.get_todo(id, proj).map_err(db_err)?;
                let combined = match existing.desc {
                    Some(prev) => format!("{prev}\n{text}"),
                    None => text.clone(),
                };
                Some(combined)
            } else {
                Some(text.clone())
            };
            db.update_desc(id, proj, desc.as_deref()).map_err(db_err)?;
            let row = db.get_todo_tree(id, proj).map_err(db_err)?;
            Ok(row.render(call.head, true))
        })
    }
}

/// Struct for the `todu tag` command
pub struct ToduTag;

impl SimplePluginCommand for ToduTag {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu tag"
    }
    fn description(&self) -> &str {
        "Set or clear the tag for a todo (pass \"none\" or \"\" to clear)"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu tag")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required(
                "tag",
                SyntaxShape::String,
                "Tag name (without #), or \"\" to clear",
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
        let tag: String = call.req(1)?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            #[cfg(feature = "remote")]
            {
                let row = db.get_todo(id, proj).map_err(db_err)?;
                if row.source != todu_db::ToduSource::Local {
                    return Err(LabeledError::new(format!(
                        "todo #{id} originates from {} — its tag is the issue identifier and cannot be changed",
                        row.source.label()
                    ))
                    .with_label("remote todo", call.positional[0].span()));
                }
            }
            let tag_val = if tag.is_empty() || tag.eq_ignore_ascii_case("none") { None } else { Some(tag.as_str()) };
            db.update_tag(id, proj, tag_val).map_err(db_err)?;
            let row = db.get_todo_tree(id, proj).map_err(db_err)?;
            Ok(row.render(call.head, true))
        })
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
        let date: String = call.req(1)?;
        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;
            let due = if date.is_empty() || date.eq_ignore_ascii_case("none") {
                None
            } else {
                parse_due(&date)?
            };
            db.update_due(id, proj, due).map_err(db_err)?;
            let row = db.get_todo_tree(id, proj).map_err(db_err)?;
            Ok(row.render(call.head, true))
        })
    }
}
