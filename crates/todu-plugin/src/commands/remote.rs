use crate::remote::{parse_github_url, parse_jira_remote};
use crate::{db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Record, Signature, Span, SyntaxShape, Type, Value};
use todu_db::ToduRemote;

fn render_remote(r: &ToduRemote, span: Span) -> Value {
    let mut rec = Record::new();
    rec.push("type", Value::string(r.remote_type.clone(), span));
    rec.push("url", Value::string(r.url.clone(), span));
    Value::record(rec, span)
}

/// Struct for the `todu remote` command
pub struct ToduRemoteList;

impl SimplePluginCommand for ToduRemoteList {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu remote"
    }

    fn description(&self) -> &str {
        "List configured remotes for the current project"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu remote")
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
        plugin.with_project(engine, call, |db, proj| {
            let span = call.head;
            let remotes = db.get_remotes(proj, None).map_err(db_err)?;
            if remotes.is_empty() {
                return Ok(Value::string(
                    "No remotes configured — add one with `todu remote add github <url>` or `todu remote add jira <url>`",
                    span,
                ));
            }
            Ok(Value::list(
                remotes.iter().map(|r| render_remote(r, span)).collect(),
                span,
            ))
        })
    }
}

/// Struct for the `todu remote add github` command
pub struct ToduRemoteAddGitHub;

impl SimplePluginCommand for ToduRemoteAddGitHub {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu remote add github"
    }

    fn description(&self) -> &str {
        "Add a GitHub repo as a remote for the current project"
    }

    fn extra_description(&self) -> &str {
        "Example: todu remote add github https://github.com/myorg/myrepo"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu remote add github")
            .required("url", SyntaxShape::String, "GitHub repo URL (https://github.com/owner/repo)")
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
        let url: String = call.req(0)?;
        parse_github_url(&url)?;

        plugin.with_project(engine, call, |db, proj| {
            let span = call.head;
            let existing = db.get_remotes(proj, Some("github")).map_err(db_err)?;
            if let Some(r) = existing.first() {
                return Err(LabeledError::new(format!(
                    "a GitHub remote is already configured for this project: {}\nRemove it first with `todu remote rm github`",
                    r.url
                )));
            }
            db.add_remote(proj, "github", &url).map_err(db_err)?;
            Ok(Value::string(format!("Added GitHub remote: {url}"), span))
        })
    }
}

/// Struct for the `todu remote add jira` command
pub struct ToduRemoteAddJira;

impl SimplePluginCommand for ToduRemoteAddJira {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu remote add jira"
    }

    fn description(&self) -> &str {
        "Add a Jira instance as a remote for the current project"
    }

    fn extra_description(&self) -> &str {
        "The project key is required as a path segment in the URL.\n\
         Example: todu remote add jira https://myorg.atlassian.net/PROJ"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu remote add jira")
            .required("url", SyntaxShape::String, "Jira URL including project key (https://myorg.atlassian.net/PROJECT)")
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
        let url: String = call.req(0)?;
        parse_jira_remote(&url)?;

        plugin.with_project(engine, call, |db, proj| {
            let span = call.head;
            let existing = db.get_remotes(proj, Some("jira")).map_err(db_err)?;
            if let Some(r) = existing.first() {
                return Err(LabeledError::new(format!(
                    "a Jira remote is already configured for this project: {}\nRemove it first with `todu remote rm jira`",
                    r.url
                )));
            }
            db.add_remote(proj, "jira", &url).map_err(db_err)?;
            Ok(Value::string(format!("Added Jira remote: {url}"), span))
        })
    }
}

/// Struct for the `todu remote rm` command
pub struct ToduRemoteRm;

impl SimplePluginCommand for ToduRemoteRm {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu remote rm"
    }

    fn description(&self) -> &str {
        "Remove a configured remote from the current project"
    }

    fn signature(&self) -> Signature {
        Signature::build("todu remote rm")
            .required("type", SyntaxShape::String, "Remote type (github or jira)")
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
        let remote_type: String = call.req(0)?;

        plugin.with_project(engine, call, |db, proj| {
            let remotes = db.get_remotes(proj, Some(&remote_type)).map_err(db_err)?;
            let remote = remotes.first().ok_or_else(|| {
                LabeledError::new(format!("no {remote_type} remote configured for this project"))
            })?;
            db.remove_remote(proj, &remote_type, &remote.url).map_err(db_err)?;
            Ok(Value::nothing(call.head))
        })
    }
}
