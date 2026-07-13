use crate::{assert_todo_exists, db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Type, Value};
use std::process::Command;

/// Struct for the `todu branch` command
pub struct ToduBranch;

impl SimplePluginCommand for ToduBranch {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu branch"
    }

    fn description(&self) -> &str {
        "Create and switch to a git branch linked to a todo item"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu branch")
            .required("id", SyntaxShape::Int, "Todu ID")
            .required(
                "branch_name",
                SyntaxShape::String,
                "Name for the new git branch",
            )
            .switch("global", "Use home directory as project", Some('g'))
            .input_output_type(Type::Nothing, Type::String)
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
        let branch_name: String = call.req(1)?;

        let cwd = engine
            .get_current_dir()
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let git_check = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&cwd)
            .output()
            .map_err(|e| LabeledError::new(format!("Failed to run git: {e}")))?;

        if !git_check.status.success() {
            return Err(LabeledError::new(
                "Not in a git repository — todu branch requires a git repo",
            ));
        }

        plugin.with_project(engine, call, |db, proj| {
            assert_todo_exists(db, id, proj, call.positional[0].span())?;

            let result = Command::new("git")
                .args(["checkout", "-b", &branch_name])
                .current_dir(&cwd)
                .output()
                .map_err(|e| LabeledError::new(format!("Failed to run git: {e}")))?;

            if !result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                return Err(LabeledError::new(format!(
                    "git checkout -b failed: {}",
                    stderr.trim()
                )));
            }

            db.update_branch(id, proj, Some(&branch_name)).map_err(db_err)?;

            Ok(Value::string(
                format!("Switched to new branch '{branch_name}'"),
                call.head,
            ))
        })
    }
}
