use crate::{assert_todo_exists, db_err, parse_inline, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{
    Category, LabeledError, PipelineData, Signature, Span, SyntaxShape, Type, Value,
};

/// Struct for the `todu add` command
pub struct ToduAdd;

impl PluginCommand for ToduAdd {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu add"
    }
    fn description(&self) -> &str {
        "Add a new todo to the project"
    }

    fn extra_description(&self) -> &str {
        "Inline syntax:\n\
           \x20 todu add \"task // description\"             — inline description\n\
           \x20 todu add \"task @2026-07-01\"                — inline due date\n\
           \x20 todu add \"task ! #work @fri // desc\"       — low priority, tag, due date\n\
           \x20 todu add \"task !! // desc\"                 — medium priority\n\
           \x20 todu add \"task ^2 // desc\"                 — subtask of todo #2\n\
           \x20 todu add \"task #work // desc\"              — tag the task\n\
          \nPriority tokens: ! = low  !! = medium  !!! = high\n\
          \nMultiple todos:\n\
           \x20 todu add \"task1\" \"task2\" \"task3\"\n\
           \x20 [ \"task1\" \"task2\" \"task3\" ] | todu add"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu add")
            .rest("tasks", SyntaxShape::String, "Task description(s)")
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_types(vec![
                (Type::Nothing, Type::Any),
                (Type::List(Box::new(Type::String)), Type::Any),
                (Type::Any, Type::Any),
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
        let tasks_arg: Vec<(String, Span)> = call
            .positional
            .iter()
            .map(|v| {
                v.as_str()
                    .map(|s| (s.to_owned(), v.span()))
                    .map_err(|e| LabeledError::new(e.to_string()))
            })
            .collect::<Result<_, _>>()?;
        let raw_tasks: Vec<(String, Span)> = if !tasks_arg.is_empty() {
            tasks_arg
        } else {
            match input {
                PipelineData::Empty => {
                    return Err(LabeledError::new(
                        "provide a task argument or pipe a list of tasks",
                    ))
                }
                // support ["task1", "task2", ...]
                _ => input
                    .into_iter()
                    .map(|v| {
                        let span = v.span();
                        v.as_str()
                            .map(|s| (s.to_owned(), span))
                            .map_err(|e| LabeledError::new(e.to_string()))
                    })
                    .collect::<Result<_, _>>()?,
            }
        };

        plugin.with_project(engine, call, |db, proj| {
            let head = call.head;
            let mut rendered = Vec::new();
            for (task_str, span) in &raw_tasks {
                let item = parse_inline(task_str)?;
                if let Some(parent_id) = item.pptid {
                    assert_todo_exists(db, parent_id, proj, *span)?;
                }
                let row = db.insert_todo(proj, &item).map_err(db_err)?;
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
