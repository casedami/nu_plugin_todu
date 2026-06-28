//! SQLite-backed persistence layer and shared domain types

mod priority;
mod row;
mod source;
mod status;

pub use priority::ToduPriority;
pub use row::ToduRow;
pub use source::ToduSource;
pub use status::ToduStatus;

use chrono::NaiveDate;
use nu_protocol::ast::{Comparison, Operator};
use rusqlite::{params, Connection, Result as SqlResult};
use std::collections::HashMap;
use std::path::Path;

use std::cmp::Ordering;

/// Maps a `std::cmp::Ordering` to a boolean result for a nu comparison `Operator`.
fn compare_ordering(ord: Ordering, operator: Operator) -> Option<bool> {
    match operator {
        Operator::Comparison(Comparison::Equal) => Some(ord == Ordering::Equal),
        Operator::Comparison(Comparison::NotEqual) => Some(ord != Ordering::Equal),
        Operator::Comparison(Comparison::LessThan) => Some(ord == Ordering::Less),
        Operator::Comparison(Comparison::LessThanOrEqual) => Some(ord != Ordering::Greater),
        Operator::Comparison(Comparison::GreaterThan) => Some(ord == Ordering::Greater),
        Operator::Comparison(Comparison::GreaterThanOrEqual) => Some(ord != Ordering::Less),
        _ => None,
    }
}

/// Intermediary struct between user input and the database
pub struct ParsedTodu {
    /// Task item
    pub task: String,
    /// Task priority
    pub priority: ToduPriority,
    /// Task due date
    pub due: Option<NaiveDate>,
    /// Optionay task description
    pub desc: String,
    /// Task parent (if any)
    pub pptid: Option<i64>,
    /// Task categorization tag
    pub tag: Option<String>,
    /// Task source
    pub source: ToduSource,
}

/// Handle for an open SQLite connection with the todu schema initialized
pub struct ToduLocalDatabase {
    conn: Connection,
}

impl ToduLocalDatabase {
    fn init(conn: Connection) -> SqlResult<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                ptid        INTEGER NOT NULL DEFAULT 0,
                project     TEXT    NOT NULL,
                task        TEXT    NOT NULL,
                status      TEXT    NOT NULL DEFAULT 'pending',
                priority    TEXT    NOT NULL DEFAULT 'unset',
                due         TEXT,
                desc        TEXT,
                created     INTEGER NOT NULL,
                pptid       INTEGER,
                tag         TEXT,
                source      TEXT    NOT NULL DEFAULT 'local',
                deleted_at  INTEGER
            );",
        )?;
        Ok(Self { conn })
    }

    /// Opens (or creates) the database file at `path` and initializes the schema
    pub fn open(path: &Path) -> SqlResult<Self> {
        if !path.exists() {
            std::fs::create_dir_all(path.parent().unwrap()).ok();
        }
        Self::init(Connection::open(path)?)
    }

    /// Returns all non-deleted todos in `project` as a nested parent-child tree, ordered by `ptid`
    pub fn get_live_todos(&self, project: &str) -> SqlResult<Vec<ToduRow>> {
        let sql = format!(
            "SELECT {} FROM todos WHERE project = ?1 AND deleted_at IS NULL ORDER BY ptid",
            ToduRow::COLS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let flat = stmt
            .query_map(params![project], ToduRow::from_sql)?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(build_tree(flat))
    }

    /// Returns `true` if a todo with `ptid` exists in `project` (including soft-deleted rows)
    pub fn todo_exists(&self, ptid: i64, project: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM todos WHERE ptid = ?1 AND project = ?2",
            params![ptid, project],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Returns a single todo item given a project and a `ptid`
    pub fn get_todo(&self, ptid: i64, project: &str) -> SqlResult<ToduRow> {
        let sql = format!(
            "SELECT {} FROM todos WHERE ptid = ?1 AND project = ?2",
            ToduRow::COLS,
        );
        self.conn
            .prepare(&sql)?
            .query_row(params![ptid, project], ToduRow::from_sql)
    }

    /// Sets the status of `ptid` and propagates the change up to ancestor tasks
    pub fn set_todo_status(&self, ptid: i64, project: &str, status: ToduStatus) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET status = ?1 WHERE ptid = ?2 AND project = ?3",
            params![status, ptid, project],
        )?;
        self.sync_parent_status(ptid, project)
    }

    /// Recursively updates the parent's derived status based on its children
    fn sync_parent_status(&self, ptid: i64, project: &str) -> SqlResult<()> {
        let pptid: Option<i64> = self.conn.query_row(
            "SELECT pptid FROM todos WHERE ptid = ?1 AND project = ?2",
            params![ptid, project],
            |row| row.get(0),
        )?;

        let Some(parent_id) = pptid else {
            return Ok(());
        };

        let any_children_in_progress: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM todos WHERE pptid = ?1 AND project = ?2 AND status = ?3 AND deleted_at IS NULL)",
            params![parent_id, project, ToduStatus::InProgress],
            |row| row.get(0),
        )?;

        let all_children_done: bool = self.conn.query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM todos WHERE pptid = ?1 AND project = ?2 AND status != ?3 AND deleted_at IS NULL)",
            params![parent_id, project, ToduStatus::Done],
            |row| row.get(0),
        )?;

        let new_status = if any_children_in_progress {
            ToduStatus::InProgress
        } else if all_children_done {
            ToduStatus::InReview
        } else {
            ToduStatus::Pending
        };

        self.conn.execute(
            "UPDATE todos SET status = ?1 WHERE ptid = ?2 AND project = ?3",
            params![new_status, parent_id, project],
        )?;

        self.sync_parent_status(parent_id, project)
    }

    /// Inserts a new todo into `project`, auto-assigning and returning the `ptid`
    pub fn insert_todo(&self, project: &str, todo: &ParsedTodu) -> SqlResult<ToduRow> {
        let next_ptid: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(ptid), 0) + 1 FROM todos WHERE project = ?1",
            params![project],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO todos (project, task, priority, status, due, desc, ptid, pptid, created, tag, source)
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, unixepoch('now'), ?8, ?9)",
            params![project, todo.task, todo.priority, todo.due, todo.desc, next_ptid, todo.pptid, todo.tag, todo.source.label()],
        )?;
        self.get_todo(next_ptid, project)
    }

    /// Updates the tag on todo `ptid`
    pub fn update_tag(&self, ptid: i64, project: &str, tag: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET tag = ?1 WHERE ptid = ?2 AND project = ?3",
            params![tag, ptid, project],
        )?;
        Ok(())
    }

    /// Updates the description of todo `ptid`
    pub fn update_desc(&self, ptid: i64, project: &str, desc: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET desc = ?1 WHERE ptid = ?2 AND project = ?3",
            params![desc, ptid, project],
        )?;
        Ok(())
    }

    /// Updates the due date of todo `ptid`
    pub fn update_due(&self, ptid: i64, project: &str, due: Option<NaiveDate>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET due = ?1 WHERE ptid = ?2 AND project = ?3",
            params![due, ptid, project],
        )?;
        Ok(())
    }

    /// Soft-deletes all `Done` and `Stopped` todos in `project`. Returns the number of rows affected
    pub fn clear_done(&self, project: &str) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE todos SET deleted_at = unixepoch('now')
             WHERE status IN (?2, ?3) AND project = ?1 AND deleted_at IS NULL",
            params![project, ToduStatus::Done, ToduStatus::Stopped],
        )
    }

    /// Soft-deletes every todo in `project`. Returns the number of rows affected
    pub fn clear_all(&self, project: &str) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE todos SET deleted_at = unixepoch('now') WHERE project = ?1 AND deleted_at IS NULL",
            params![project],
        )
    }

    /// Soft-deletes a single todo by `ptid`. Returns the number of rows affected (0 if not found)
    pub fn delete_todo(&self, ptid: i64, project: &str) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE todos SET deleted_at = unixepoch('now') WHERE ptid = ?1 AND project = ?2 AND deleted_at IS NULL",
            params![ptid, project],
        )
    }

    /// Permanently removes all soft-deleted rows in `project`. Returns the number of rows deleted
    pub fn purge_deleted(&self, project: &str) -> SqlResult<usize> {
        self.conn.execute(
            "DELETE FROM todos WHERE deleted_at IS NOT NULL AND project = ?1",
            params![project],
        )
    }
}

/// Assembles a flat list of rows (ordered by `ptid`) into a parent-child tree
fn build_tree(flat: Vec<ToduRow>) -> Vec<ToduRow> {
    let mut children: HashMap<i64, Vec<ToduRow>> = HashMap::new();
    let mut roots: Vec<ToduRow> = Vec::new();
    for row in flat {
        match row.pptid {
            Some(parent) => children.entry(parent).or_default().push(row),
            None => roots.push(row),
        }
    }
    attach_children(&mut roots, &mut children);
    roots
}

/// Recursively moves children out of `children` and into their parent's `subtasks` field.
fn attach_children(tasks: &mut [ToduRow], children: &mut HashMap<i64, Vec<ToduRow>>) {
    for task in tasks.iter_mut() {
        if let Some(subs) = children.remove(&task.ptid) {
            task.subtasks = subs;
            attach_children(&mut task.subtasks, children);
        }
    }
}
