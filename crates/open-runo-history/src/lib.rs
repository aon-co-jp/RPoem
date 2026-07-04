//! `open-runo-history`: Git-like history for schema, database, and
//! configuration changes, so production changes are reviewable and
//! reversible (see README section 8).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use chrono::{DateTime, Utc};
use open_runo_core::{AppError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub id: Uuid,
    pub author: String,
    pub message: String,
    pub before: String,
    pub after: String,
    pub created_at: DateTime<Utc>,
    pub approved: bool,
}

/// An append-only, in-memory change log with rollback support. Mirrors
/// `git log` / `git revert` semantics at a much smaller scale.
#[derive(Debug, Default)]
pub struct History {
    records: Vec<ChangeRecord>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a change. Returns the new record's id for later approval /
    /// rollback.
    pub fn commit(&mut self, author: &str, message: &str, before: &str, after: &str) -> Uuid {
        let record = ChangeRecord {
            id: Uuid::new_v4(),
            author: author.to_string(),
            message: message.to_string(),
            before: before.to_string(),
            after: after.to_string(),
            created_at: Utc::now(),
            approved: false,
        };
        let id = record.id;
        self.records.push(record);
        id
    }

    pub fn approve(&mut self, id: Uuid) -> Result<()> {
        let record = self
            .records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| AppError::NotFound(format!("change record {id} not found")))?;
        record.approved = true;
        Ok(())
    }

    /// Returns the state ("after" value) that should be active if the
    /// change with the given id is rolled back — i.e. its "before" value.
    pub fn rollback_target(&self, id: Uuid) -> Result<&str> {
        self.records
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.before.as_str())
            .ok_or_else(|| AppError::NotFound(format!("change record {id} not found")))
    }

    pub fn log(&self) -> &[ChangeRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_approve_and_rollback() {
        let mut history = History::new();
        let id = history.commit("alice", "add name field", "{}", "{\"name\":true}");
        assert!(!history.log()[0].approved);

        history.approve(id).unwrap();
        assert!(history.log()[0].approved);

        assert_eq!(history.rollback_target(id).unwrap(), "{}");
    }

    #[test]
    fn approve_unknown_id_errors() {
        let mut history = History::new();
        assert!(history.approve(Uuid::new_v4()).is_err());
    }
}
