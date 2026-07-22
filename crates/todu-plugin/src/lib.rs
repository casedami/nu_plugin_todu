//! Nushell plugin that provides todo list management commands backed by a local SQLite database.

mod commands;
mod config;
mod parse;
#[cfg(feature = "remote")]
mod remote;

pub(crate) use parse::{parse_due, parse_inline};

pub use commands::*;

use config::Config;
use nu_plugin::{EngineInterface, EvaluatedCall, Plugin, PluginCommand};
use nu_protocol::{LabeledError, Span};
use std::process::Command;
use std::sync::Mutex;
use todu_db::ToduLocalDatabase;

fn get_root_directory(engine: &EngineInterface, global: bool) -> Result<String, LabeledError> {
    if global {
        Ok(std::env::var("HOME").unwrap_or_default())
    } else {
        let cwd = engine
            .get_current_dir()
            .map_err(|e| LabeledError::new(e.to_string()))?;
        let out = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&cwd)
            .output();
        Ok(match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => cwd,
        })
    }
}

fn db_err(e: impl std::fmt::Display) -> LabeledError {
    LabeledError::new(format!("Database error: {e}"))
}

fn assert_todo_exists(
    db: &ToduLocalDatabase,
    ptid: i64,
    project: &str,
    span: Span,
) -> Result<(), LabeledError> {
    if !db.todo_exists(ptid, project).map_err(db_err)? {
        return Err(
            LabeledError::new(format!("No todo #{ptid} in this project"))
                .with_label("id given here", span),
        );
    }
    Ok(())
}

/// The root plugin type. Holds a lazily-initialized [`Config`] and [`ToduLocalDatabase`] behind a
/// [`Mutex`] so they can be shared across invocations within a nushell process.
pub struct ToduPlugin {
    pub(crate) state: Mutex<Option<(Config, ToduLocalDatabase)>>,
}

impl ToduPlugin {
    /// Creates a new ToduPlugin instance with a lazily-initialized state.
    pub fn lazy() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    /// Runs `f` with the open database and a pre-resolved project path. Use this when the project
    /// is already known so as to avoid re-running `git rev-parse`.
    pub fn with_db<F, T>(&self, project: &str, f: F) -> Result<T, LabeledError>
    where
        F: FnOnce(&ToduLocalDatabase, &str) -> Result<T, LabeledError>,
    {
        let guard = self.state.lock().unwrap();
        let (_, db) = guard.as_ref().expect("with_db called before with_project");
        f(db, project)
    }

    /// Runs `f` with a reference to the config and the open database, initializing both on first
    /// call. Avoid using this in repeated operations where the project directory is guaranteed to
    /// be unchanged, use `with_db` instead.
    pub fn with_project<F, T>(
        &self,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        f: F,
    ) -> Result<T, LabeledError>
    where
        F: FnOnce(&ToduLocalDatabase, &str) -> Result<T, LabeledError>,
    {
        let mut guard = self.state.lock().unwrap();
        if guard.is_none() {
            let cfg = Config::from_engine(engine);
            let db = ToduLocalDatabase::open(&cfg.db_path).map_err(db_err)?;
            *guard = Some((cfg, db));
        }
        let (cfg, db) = guard.as_ref().unwrap();
        let global = call.has_flag("global")? || cfg.default_global;
        let project = get_root_directory(engine, global)?;
        f(db, &project)
    }
}

impl Plugin for ToduPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(ToduList),
            Box::new(ToduGet),
            Box::new(ToduBranch),
            Box::new(ToduAdd),
            Box::new(ToduDone),
            Box::new(ToduStart),
            Box::new(ToduStop),
            Box::new(ToduPause),
            Box::new(ToduReopen),
            Box::new(ToduTitle),
            Box::new(ToduDesc),
            Box::new(ToduDue),
            Box::new(ToduTag),
            Box::new(ToduPriorityCmd),
            Box::new(ToduMove),
            Box::new(ToduClear),
            #[cfg(feature = "remote")]
            Box::new(ToduRemoteList),
            #[cfg(feature = "remote")]
            Box::new(ToduRemoteAddGitHub),
            #[cfg(feature = "remote")]
            Box::new(ToduRemoteAddJira),
            #[cfg(feature = "remote")]
            Box::new(ToduRemoteRm),
            #[cfg(feature = "remote")]
            Box::new(ToduPullGitHub),
            #[cfg(feature = "remote")]
            Box::new(ToduPullJira),
        ]
    }
}
