//! SQLite-backed persistence layer and shared domain types

mod priority;
#[cfg(feature = "remote")]
mod remote;
mod row;
mod source;
mod status;

pub use priority::ToduPriority;
#[cfg(feature = "remote")]
pub use remote::ToduRemote;
pub use row::ToduRow;
pub use source::ToduSource;
pub use status::ToduStatus;

use chrono::NaiveDate;
use nu_protocol::ast::{Comparison, Operator};
#[cfg(feature = "remote")]
use rusqlite::OptionalExtension;
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
    /// Task title
    pub title: String,
    /// Task priority
    pub priority: Option<ToduPriority>,
    /// Task due date
    pub due: Option<NaiveDate>,
    /// Optional task description
    pub desc: Option<String>,
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
                title       TEXT    NOT NULL,
                status      TEXT    NOT NULL DEFAULT 'pending',
                priority    TEXT,
                due         TEXT,
                desc        TEXT,
                created     INTEGER NOT NULL,
                pptid       INTEGER,
                tag         TEXT,
                source      TEXT    NOT NULL DEFAULT 'local',
                deleted_at  INTEGER,
                branch      TEXT
            );",
        )?;

        #[cfg(feature = "remote")]
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS remotes (
                project     TEXT NOT NULL,
                type        TEXT NOT NULL,
                url         TEXT NOT NULL,
                PRIMARY KEY (project, type, url)
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

    /// Returns all non-deleted todos in `project` as a nested parent-child tree, sorted by status, priority, then `ptid`
    pub fn get_live_todos(&self, project: &str) -> SqlResult<Vec<ToduRow>> {
        let sql = format!(
            "SELECT {} FROM todos WHERE project = ?1 AND deleted_at IS NULL ORDER BY ptid",
            ToduRow::COLS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let flat = stmt
            .query_map(params![project], ToduRow::from_sql)?
            .collect::<SqlResult<Vec<_>>>()?;
        let mut tree = build_tree(flat);
        sort_tree(&mut tree);
        Ok(tree)
    }

    /// Returns `true` if a live (non-deleted) todo with `ptid` exists in `project`
    pub fn todo_exists(&self, ptid: i64, project: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM todos WHERE ptid = ?1 AND project = ?2 AND deleted_at IS NULL",
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

    /// Returns a single todo item with its full subtask tree
    pub fn get_todo_tree(&self, ptid: i64, project: &str) -> SqlResult<ToduRow> {
        let sql = format!(
            "SELECT {} FROM todos WHERE project = ?1 AND deleted_at IS NULL ORDER BY ptid",
            ToduRow::COLS
        );
        let flat = self
            .conn
            .prepare(&sql)?
            .query_map(params![project], ToduRow::from_sql)?
            .collect::<SqlResult<Vec<_>>>()?;
        let mut tree = build_tree(flat);
        sort_tree(&mut tree);
        find_in_tree(&mut tree, ptid).ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    /// Sets the status of `ptid` and propagates the change up to ancestor tasks
    pub fn set_todo_status(&self, ptid: i64, project: &str, status: ToduStatus) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET status = ?1 WHERE ptid = ?2 AND project = ?3",
            params![status, ptid, project],
        )?;
        self.sync_parent_status(ptid, project)
    }

    fn sync_parent_status(&self, ptid: i64, project: &str) -> SqlResult<()> {
        let pptid: Option<i64> = self.conn.query_row(
            "SELECT pptid FROM todos WHERE ptid = ?1 AND project = ?2",
            params![ptid, project],
            |row| row.get(0),
        )?;

        let Some(parent_id) = pptid else {
            return Ok(());
        };

        let all_children_done: bool = self.conn.query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM todos WHERE pptid = ?1 AND project = ?2 AND status != ?3 AND deleted_at IS NULL)",
            params![parent_id, project, ToduStatus::Done],
            |row| row.get(0),
        )?;

        let any_children_in_progress_or_done: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM todos WHERE pptid = ?1 AND project = ?2 AND status IN (?3, ?4) AND deleted_at IS NULL)",
            params![parent_id, project, ToduStatus::InProgress, ToduStatus::Done],
            |row| row.get(0),
        )?;

        let new_status = if all_children_done {
            ToduStatus::InReview
        } else if any_children_in_progress_or_done {
            ToduStatus::InProgress
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
            "INSERT INTO todos (project, title, priority, status, due, desc, ptid, pptid, created, tag, source)
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, unixepoch('now'), ?8, ?9)",
            params![project, todo.title, todo.priority.map(|p| p.label()), todo.due, todo.desc, next_ptid, todo.pptid, todo.tag, todo.source.label()],
        )?;
        self.get_todo(next_ptid, project)
    }

    /// Returns the first non-deleted todo in `project` with the given `tag` and `source`, or `None`
    #[cfg(feature = "remote")]
    pub fn find_todo_by_tag_and_source(
        &self,
        project: &str,
        tag: &str,
        source: ToduSource,
    ) -> SqlResult<Option<ToduRow>> {
        let sql = format!(
            "SELECT {} FROM todos WHERE project = ?1 AND tag = ?2 AND source = ?3 AND deleted_at IS NULL LIMIT 1",
            ToduRow::COLS
        );
        self.conn
            .prepare(&sql)?
            .query_row(params![project, tag, source.label()], ToduRow::from_sql)
            .optional()
    }

    /// Returns all configured remotes for `project`, optionally filtered to `remote_type`
    #[cfg(feature = "remote")]
    pub fn get_remotes(
        &self,
        project: &str,
        remote_type: Option<&str>,
    ) -> SqlResult<Vec<ToduRemote>> {
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match remote_type {
            Some(t) => (
                "SELECT type, url FROM remotes WHERE project = ?1 AND type = ?2 ORDER BY type, url"
                    .to_owned(),
                vec![Box::new(project.to_owned()), Box::new(t.to_owned())],
            ),
            None => (
                "SELECT type, url FROM remotes WHERE project = ?1 ORDER BY type, url".to_owned(),
                vec![Box::new(project.to_owned())],
            ),
        };
        self.conn
            .prepare(&sql)?
            .query_map(rusqlite::params_from_iter(params), |row| {
                Ok(ToduRemote {
                    remote_type: row.get(0)?,
                    url: row.get(1)?,
                })
            })?
            .collect()
    }

    /// Adds a remote for `project`. Returns `true` if inserted, `false` if it already existed
    #[cfg(feature = "remote")]
    pub fn add_remote(&self, project: &str, remote_type: &str, url: &str) -> SqlResult<bool> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO remotes (project, type, url) VALUES (?1, ?2, ?3)",
            params![project, remote_type, url],
        )?;
        Ok(n > 0)
    }

    /// Removes a remote for `project`. Returns the number of rows deleted (0 if not found)
    #[cfg(feature = "remote")]
    pub fn remove_remote(&self, project: &str, remote_type: &str, url: &str) -> SqlResult<usize> {
        self.conn.execute(
            "DELETE FROM remotes WHERE project = ?1 AND type = ?2 AND url = ?3",
            params![project, remote_type, url],
        )
    }

    /// Updates the title of todo `ptid`
    pub fn update_title(&self, ptid: i64, project: &str, title: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET title = ?1 WHERE ptid = ?2 AND project = ?3",
            params![title, ptid, project],
        )?;
        Ok(())
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
    pub fn update_desc(&self, ptid: i64, project: &str, desc: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET desc = ?1 WHERE ptid = ?2 AND project = ?3",
            params![desc, ptid, project],
        )?;
        Ok(())
    }

    /// Updates the priority of todo `ptid`
    pub fn update_priority(&self, ptid: i64, project: &str, priority: Option<ToduPriority>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET priority = ?1 WHERE ptid = ?2 AND project = ?3",
            params![priority, ptid, project],
        )?;
        Ok(())
    }

    /// Updates the parent of todo `ptid`. Pass `None` to make it a root-level task
    pub fn update_parent(&self, ptid: i64, project: &str, pptid: Option<i64>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET pptid = ?1 WHERE ptid = ?2 AND project = ?3",
            params![pptid, ptid, project],
        )?;
        Ok(())
    }

    /// Returns `true` if `potential_ancestor` is an ancestor of `of_ptid` in `project`
    pub fn is_ancestor_of(
        &self,
        potential_ancestor: i64,
        of_ptid: i64,
        project: &str,
    ) -> SqlResult<bool> {
        let mut current = of_ptid;
        loop {
            let parent: Option<i64> = self.conn.query_row(
                "SELECT pptid FROM todos WHERE ptid = ?1 AND project = ?2",
                params![current, project],
                |row| row.get(0),
            )?;
            match parent {
                None => return Ok(false),
                Some(p) if p == potential_ancestor => return Ok(true),
                Some(p) => current = p,
            }
        }
    }

    /// Updates the due date of todo `ptid`
    pub fn update_due(&self, ptid: i64, project: &str, due: Option<NaiveDate>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET due = ?1 WHERE ptid = ?2 AND project = ?3",
            params![due, ptid, project],
        )?;
        Ok(())
    }

    /// Updates the branch name of todo `ptid`
    pub fn update_branch(&self, ptid: i64, project: &str, branch: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE todos SET branch = ?1 WHERE ptid = ?2 AND project = ?3",
            params![branch, ptid, project],
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

/// Sorts a tree of todos by status, priority, then `ptid`, recursively
fn sort_tree(tasks: &mut [ToduRow]) {
    tasks.sort_by_key(|t| (std::cmp::Reverse(t.status), std::cmp::Reverse(t.priority), t.ptid));
    for task in tasks.iter_mut() {
        sort_tree(&mut task.subtasks);
    }
}

/// Removes and returns the node with `ptid` from anywhere in the tree
fn find_in_tree(nodes: &mut Vec<ToduRow>, ptid: i64) -> Option<ToduRow> {
    for i in 0..nodes.len() {
        if nodes[i].ptid == ptid {
            return Some(nodes.remove(i));
        }
        if let Some(found) = find_in_tree(&mut nodes[i].subtasks, ptid) {
            return Some(found);
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::{sort_tree, ToduPriority, ToduRow, ToduSource, ToduStatus};
    use chrono::Utc;

    fn make_row(ptid: i64, status: ToduStatus, priority: Option<ToduPriority>) -> ToduRow {
        ToduRow {
            ptid,
            status,
            priority,
            title: String::new(),
            due: None,
            desc: None,
            created: Utc::now(),
            pptid: None,
            tag: None,
            branch: None,
            source: ToduSource::Local,
            subtasks: vec![],
        }
    }

    fn ptids(rows: &[ToduRow]) -> Vec<i64> {
        rows.iter().map(|r| r.ptid).collect()
    }

    #[test]
    fn sort_by_status() {
        let mut rows = vec![
            make_row(1, ToduStatus::Done, None),
            make_row(2, ToduStatus::Pending, None),
            make_row(3, ToduStatus::InProgress, None),
        ];
        sort_tree(&mut rows);
        assert_eq!(ptids(&rows), vec![3, 2, 1]);
    }

    #[test]
    fn sort_by_priority_within_status() {
        let mut rows = vec![
            make_row(1, ToduStatus::Pending, None),
            make_row(2, ToduStatus::Pending, Some(ToduPriority::Low)),
            make_row(3, ToduStatus::Pending, Some(ToduPriority::High)),
            make_row(4, ToduStatus::Pending, Some(ToduPriority::Medium)),
        ];
        sort_tree(&mut rows);
        assert_eq!(ptids(&rows), vec![3, 4, 2, 1]);
    }

    #[test]
    fn sort_by_ptid_within_status_and_priority() {
        let mut rows = vec![
            make_row(5, ToduStatus::Pending, Some(ToduPriority::High)),
            make_row(2, ToduStatus::Pending, Some(ToduPriority::High)),
            make_row(8, ToduStatus::Pending, Some(ToduPriority::High)),
        ];
        sort_tree(&mut rows);
        assert_eq!(ptids(&rows), vec![2, 5, 8]);
    }

    #[test]
    fn sort_subtasks_recursively() {
        let mut parent = make_row(1, ToduStatus::InProgress, None);
        parent.subtasks = vec![
            make_row(10, ToduStatus::Done, None),
            make_row(11, ToduStatus::Pending, Some(ToduPriority::High)),
            make_row(12, ToduStatus::Pending, Some(ToduPriority::Low)),
        ];
        let mut rows = vec![parent];
        sort_tree(&mut rows);
        assert_eq!(ptids(&rows[0].subtasks), vec![11, 12, 10]);
    }
}
