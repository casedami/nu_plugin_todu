use nu_plugin::EngineInterface;
use nu_protocol::LabeledError;
use serde::{Deserialize, Serialize};
use todu_db::{ToduSource, ToduStatus};

// ── config helpers ────────────────────────────────────────────────────────────

/// Extracts a required string at `section.key` from the plugin config record
pub(crate) fn cfg_str(
    cfg: &nu_protocol::Value,
    section: &str,
    key: &str,
) -> Result<String, LabeledError> {
    cfg.as_record()
        .ok()
        .and_then(|r| r.get(section))
        .and_then(|sec| sec.as_record().ok())
        .and_then(|r| r.get(key))
        .and_then(|v| v.as_str().ok().map(|s| s.to_owned()))
        .ok_or_else(|| {
            LabeledError::new(format!(
                "missing config: plugins.todu.{section}.{key} must be a string"
            ))
        })
}

/// Extracts an optional string at `section.key` from the plugin config record
pub(crate) fn cfg_str_opt(
    cfg: &nu_protocol::Value,
    section: &str,
    key: &str,
) -> Option<String> {
    cfg.as_record()
        .ok()
        .and_then(|r| r.get(section))
        .and_then(|sec| sec.as_record().ok())
        .and_then(|r| r.get(key))
        .and_then(|v| v.as_str().ok().map(|s| s.to_owned()))
}

/// Reads and trims a token from `path`, expanding a leading `~/` to `$HOME`
fn read_token_file(path: &str) -> Result<String, LabeledError> {
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{rest}", std::env::var("HOME").unwrap_or_default())
    } else {
        path.to_owned()
    };
    std::fs::read_to_string(&expanded)
        .map(|s| s.trim().to_owned())
        .map_err(|e| LabeledError::new(format!("could not read token file `{expanded}`: {e}")))
}

/// Resolves a token for `section` by checking `token_file` in config first, then the OS keychain
/// (service = `"todu-{section}"`, account = `keyring_account`).
/// `cfg` may be `None` when no plugin config is set — keychain is tried directly in that case.
pub(crate) fn resolve_token(
    cfg: Option<&nu_protocol::Value>,
    section: &str,
    keyring_account: &str,
) -> Result<String, LabeledError> {
    if let Some(cfg) = cfg {
        if let Some(path) = cfg_str_opt(cfg, section, "token_file") {
            return read_token_file(&path);
        }
    }

    let service = format!("todu-{section}");
    keyring::Entry::new(&service, keyring_account)
        .and_then(|e| e.get_password())
        .map_err(|e| {
            LabeledError::new(format!(
                "no token_file set and keychain lookup failed ({e})\n\
                 Set plugins.todu.{section}.token_file in config.nu, or store the token in \
                 the OS keychain under service=\"todu-{section}\" account=\"{keyring_account}\""
            ))
        })
}

/// Splits a Jira remote URL into `(base_url, project_key)`; expects `https://host/PROJECT`
pub(crate) fn parse_jira_remote(url: &str) -> Result<(&str, &str), LabeledError> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| LabeledError::new(format!("invalid Jira URL: `{url}`")))?;
    let slash = after_scheme.find('/').ok_or_else(|| {
        LabeledError::new(format!(
            "Jira URL must include a project key: `{url}` — use https://host/PROJECT"
        ))
    })?;
    let base = &url[..url.len() - after_scheme.len() + slash];
    let project = url[url.len() - after_scheme.len() + slash + 1..].trim_matches('/');
    if project.is_empty() {
        return Err(LabeledError::new(format!(
            "Jira URL must include a project key: `{url}` — use https://host/PROJECT"
        )));
    }
    Ok((base, project))
}

/// Parses `owner` and `repo` out of a `https://github.com/owner/repo` URL
pub(crate) fn parse_github_url(url: &str) -> Result<(&str, &str), LabeledError> {
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .ok_or_else(|| LabeledError::new(format!("not a GitHub URL: `{url}`")))?;
    path.split_once('/')
        .map(|(owner, repo)| (owner, repo.trim_end_matches('/')))
        .ok_or_else(|| {
            LabeledError::new(format!(
                "GitHub URL must include owner and repo: `{url}`"
            ))
        })
}

// ── GitHub ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GitHubPatch {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_reason: Option<&'static str>,
}

/// Closes or reopens a GitHub issue to reflect `status`
fn push_github(
    token: &str,
    owner: &str,
    repo: &str,
    issue_number: u64,
    status: ToduStatus,
) -> Result<(), LabeledError> {
    let patch = match status {
        ToduStatus::Done => GitHubPatch {
            state: "closed",
            state_reason: Some("completed"),
        },
        ToduStatus::Stopped => GitHubPatch {
            state: "closed",
            state_reason: Some("not_planned"),
        },
        _ => GitHubPatch {
            state: "open",
            state_reason: None,
        },
    };

    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues/{issue_number}");
    let resp = client
        .patch(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "todu-plugin")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&patch)
        .send()
        .map_err(|e| LabeledError::new(format!("GitHub request failed: {e}")))?;

    if !resp.status().is_success() {
        let http_status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(LabeledError::new(format!(
            "GitHub API returned {http_status}: {body}"
        )));
    }
    Ok(())
}

// ── Jira ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JiraTransitionsResponse {
    transitions: Vec<JiraTransition>,
}

#[derive(Deserialize)]
struct JiraTransition {
    id: String,
    to: JiraTransitionTarget,
}

#[derive(Deserialize)]
struct JiraTransitionTarget {
    #[serde(rename = "statusCategory")]
    status_category: JiraStatusCategory,
}

#[derive(Deserialize)]
struct JiraStatusCategory {
    key: String,
}

/// Maps `status` to the Jira status category key used when matching available transitions
fn jira_category_for(status: ToduStatus) -> &'static str {
    match status {
        ToduStatus::Done | ToduStatus::Stopped => "done",
        ToduStatus::InProgress | ToduStatus::InReview => "indeterminate",
        ToduStatus::Pending | ToduStatus::Paused => "new",
    }
}

/// Transitions a Jira issue to the status category matching `status`
fn push_jira(
    base_url: &str,
    email: &str,
    token: &str,
    issue_key: &str,
    status: ToduStatus,
) -> Result<(), LabeledError> {
    let target_category = jira_category_for(status);
    let client = reqwest::blocking::Client::new();
    let base = base_url.trim_end_matches('/');

    let transitions_url = format!("{base}/rest/api/3/issue/{issue_key}/transitions");
    let resp = client
        .get(&transitions_url)
        .basic_auth(email, Some(token))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| LabeledError::new(format!("Jira request failed: {e}")))?;

    if !resp.status().is_success() {
        let http_status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(LabeledError::new(format!(
            "Jira transitions API returned {http_status}: {body}"
        )));
    }

    let transitions = resp
        .json::<JiraTransitionsResponse>()
        .map_err(|e| LabeledError::new(format!("Failed to parse Jira transitions: {e}")))?
        .transitions;

    let transition = transitions
        .iter()
        .find(|t| t.to.status_category.key == target_category)
        .ok_or_else(|| {
            LabeledError::new(format!(
                "No Jira transition to status category \"{target_category}\" available on {issue_key}"
            ))
        })?;

    let body = serde_json::json!({"transition": {"id": &transition.id}});
    let resp = client
        .post(&transitions_url)
        .basic_auth(email, Some(token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| LabeledError::new(format!("Jira request failed: {e}")))?;

    if !resp.status().is_success() {
        let http_status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(LabeledError::new(format!(
            "Jira transition API returned {http_status}: {body}"
        )));
    }
    Ok(())
}

// ── dispatch ──────────────────────────────────────────────────────────────────

/// Pushes a local status change back to the originating remote. No-op for local todos.
pub(crate) fn push_status(
    engine: &EngineInterface,
    source: ToduSource,
    tag: Option<&str>,
    new_status: ToduStatus,
) -> Result<(), LabeledError> {
    let Some(tag) = tag else {
        return Ok(());
    };

    match source {
        ToduSource::Local => Ok(()),

        ToduSource::GitHub(repo_url) => {
            let issue_number: u64 = tag
                .strip_prefix('#')
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| LabeledError::new(format!("invalid GitHub issue tag: `{tag}`")))?;

            let (owner, repo) = parse_github_url(&repo_url)?;

            let cfg = engine
                .get_plugin_config()
                .map_err(|e| LabeledError::new(e.to_string()))
                .ok()
                .flatten();
            let token = resolve_token(cfg.as_ref(), "github", &format!("{owner}/{repo}"))?;

            push_github(&token, owner, repo, issue_number, new_status)
        }

        ToduSource::Jira(remote_url) => {
            let (base_url, _project) = parse_jira_remote(&remote_url)?;

            let cfg = engine
                .get_plugin_config()
                .map_err(|e| LabeledError::new(e.to_string()))?
                .ok_or_else(|| LabeledError::new("todu plugin config not found"))?;

            let email = cfg_str(&cfg, "jira", "email")?;
            let token = resolve_token(Some(&cfg), "jira", &email)?;

            push_jira(base_url, &email, &token, tag, new_status)
        }
    }
}
