use crate::remote::{cfg_str, parse_github_url, parse_jira_remote, resolve_token};
use crate::{db_err, ToduPlugin};
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, Type, Value};
use serde::Deserialize;
use todu_db::{ParsedTodu, ToduSource};

#[derive(Deserialize)]
struct GitHubIssue {
    number: u64,
    title: String,
    body: Option<String>,
}

/// Fetches open issues from `owner/repo` via the GitHub REST API
fn fetch_github_issues(
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<GitHubIssue>, LabeledError> {
    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues");
    let resp = client
        .get(&url)
        .query(&[("state", "open"), ("per_page", "100")])
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "todu-plugin")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|e| LabeledError::new(format!("GitHub request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(LabeledError::new(format!(
            "GitHub API returned {status}: {body}"
        )));
    }

    resp.json::<Vec<GitHubIssue>>()
        .map_err(|e| LabeledError::new(format!("Failed to parse GitHub response: {e}")))
}

/// Struct for the `todu pull github` command
pub struct ToduPullGitHub;

impl SimplePluginCommand for ToduPullGitHub {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu pull github"
    }

    fn description(&self) -> &str {
        "Pull open issues from all configured GitHub remotes into the project"
    }

    fn extra_description(&self) -> &str {
        "Add remotes first with `todu remote add github <url>`.\n\n\
         Tokens are resolved per remote via token_file or the OS keychain\n\
         (service = \"todu-github\", account = \"{owner}/{repo}\").\n\n\
         The issue number is stored in the tag column (e.g. \"#42\").\n\
         Status changes (todu done, todu start, etc.) are automatically pushed back to GitHub."
    }

    fn signature(&self) -> Signature {
        Signature::build("todu pull github")
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
        let cfg = engine
            .get_plugin_config()
            .map_err(|e| LabeledError::new(e.to_string()))
            .ok()
            .flatten();

        plugin.with_project(engine, call, |db, proj| {
            let span = call.head;
            let remotes = db.get_remotes(proj, Some("github")).map_err(db_err)?;

            if remotes.is_empty() {
                return Err(LabeledError::new(
                    "no GitHub remotes configured — run `todu remote add github <url>`",
                ));
            }

            let mut imported = Vec::new();
            let mut skipped: u64 = 0;

            for remote in &remotes {
                let (owner, repo) = parse_github_url(&remote.url)?;
                let token = resolve_token(cfg.as_ref(), "github", &format!("{owner}/{repo}"))?;

                for issue in fetch_github_issues(&token, owner, repo)? {
                    let tag = format!("#{}", issue.number);
                    let source = ToduSource::GitHub(remote.url.clone());
                    if db
                        .find_todo_by_tag_and_source(proj, &tag, source.clone())
                        .map_err(db_err)?
                        .is_some()
                    {
                        skipped += 1;
                        continue;
                    }
                    let row = db
                        .insert_todo(
                            proj,
                            &ParsedTodu {
                                title: issue.title,
                                priority: None,
                                due: None,
                                desc: issue.body.filter(|b| !b.is_empty()),
                                pptid: None,
                                tag: Some(tag),
                                source,
                            },
                        )
                        .map_err(db_err)?;
                    imported.push(row.render_short(span));
                }
            }

            let summary = format!(
                "GitHub pull: {} imported, {} already present",
                imported.len(),
                skipped
            );
            if imported.is_empty() {
                Ok(Value::string(summary, span))
            } else {
                let mut out = vec![Value::string(summary, span)];
                out.extend(imported);
                Ok(Value::list(out, span))
            }
        })
    }
}

#[derive(Deserialize)]
struct JiraSearchResult {
    issues: Vec<JiraIssue>,
}

#[derive(Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Deserialize)]
struct JiraFields {
    summary: String,
}

/// Fetches assigned, non-done issues via the Jira REST API scoped to `project`
fn fetch_jira_issues(
    base_url: &str,
    email: &str,
    token: &str,
    project: &str,
) -> Result<Vec<JiraIssue>, LabeledError> {
    let jql =
        format!("project = {project} AND assignee = currentUser() AND statusCategory != Done");

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/rest/api/3/search", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .query(&[("jql", &jql), ("maxResults", &"100".to_owned())])
        .basic_auth(email, Some(token))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| LabeledError::new(format!("Jira request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(LabeledError::new(format!(
            "Jira API returned {status}: {body}"
        )));
    }

    resp.json::<JiraSearchResult>()
        .map_err(|e| LabeledError::new(format!("Failed to parse Jira response: {e}")))
        .map(|r| r.issues)
}

/// Struct for the `todu pull jira` command
pub struct ToduPullJira;

impl SimplePluginCommand for ToduPullJira {
    type Plugin = ToduPlugin;

    fn name(&self) -> &str {
        "todu pull jira"
    }

    fn description(&self) -> &str {
        "Pull assigned issues from all configured Jira remotes into the project"
    }

    fn extra_description(&self) -> &str {
        "Add remotes first with `todu remote add jira <url>`.\n\n\
         Requires in config.nu:\n\
         $env.config.plugins.todu = {\n\
         \x20   jira: {\n\
         \x20       email:      \"me@example.com\"\n\
         \x20       project:    \"PROJ\"  # optional — omit to search all projects\n\
         \x20       token_file: \"~/.config/todu/jira_token\"\n\
         \x20   }\n\
         }\n\n\
         The Jira issue key is stored in the tag column (e.g. \"PROJ-123\").\n\
         Status changes (todu done, todu start, etc.) are automatically pushed back via Jira transitions."
    }

    fn signature(&self) -> Signature {
        Signature::build("todu pull jira")
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
        let cfg = engine
            .get_plugin_config()
            .map_err(|e| LabeledError::new(e.to_string()))?
            .ok_or_else(|| LabeledError::new("todu plugin config not found"))?;

        let email = cfg_str(&cfg, "jira", "email")?;
        let token = resolve_token(Some(&cfg), "jira", &email)?;

        plugin.with_project(engine, call, |db, proj| {
            let span = call.head;
            let remotes = db.get_remotes(proj, Some("jira")).map_err(db_err)?;

            if remotes.is_empty() {
                return Err(LabeledError::new(
                    "no Jira remotes configured — run `todu remote add jira <url>`",
                ));
            }

            let mut imported = Vec::new();
            let mut skipped: u64 = 0;

            for remote in &remotes {
                let (base_url, project) = parse_jira_remote(&remote.url)?;
                for issue in fetch_jira_issues(base_url, &email, &token, project)? {
                    let tag = issue.key;
                    let source = ToduSource::Jira(remote.url.clone());
                    if db
                        .find_todo_by_tag_and_source(proj, &tag, source.clone())
                        .map_err(db_err)?
                        .is_some()
                    {
                        skipped += 1;
                        continue;
                    }
                    let row = db
                        .insert_todo(
                            proj,
                            &ParsedTodu {
                                title: issue.fields.summary,
                                priority: None,
                                due: None,
                                desc: None,
                                pptid: None,
                                tag: Some(tag),
                                source,
                            },
                        )
                        .map_err(db_err)?;
                    imported.push(row.render_short(span));
                }
            }

            let summary = format!(
                "Jira pull: {} imported, {} already present",
                imported.len(),
                skipped
            );
            if imported.is_empty() {
                Ok(Value::string(summary, span))
            } else {
                let mut out = vec![Value::string(summary, span)];
                out.extend(imported);
                Ok(Value::list(out, span))
            }
        })
    }
}
