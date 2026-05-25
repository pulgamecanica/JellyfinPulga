use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::models::*;

pub struct ChatDb {
    conn: Mutex<Connection>,
}

impl ChatDb {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chat_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                room_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                username TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_chat_room ON chat_messages(room_id, created_at);

            CREATE TABLE IF NOT EXISTS private_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_user_id TEXT NOT NULL,
                from_username TEXT NOT NULL,
                to_user_id TEXT NOT NULL,
                to_username TEXT NOT NULL,
                content TEXT NOT NULL,
                read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_pm_to ON private_messages(to_user_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_pm_from ON private_messages(from_user_id, created_at);

            CREATE TABLE IF NOT EXISTS content_reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id TEXT NOT NULL,
                item_name TEXT NOT NULL,
                reporter_id TEXT NOT NULL,
                reporter_name TEXT NOT NULL,
                reason TEXT NOT NULL,
                details TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_reports_status ON content_reports(status);
            CREATE INDEX IF NOT EXISTS idx_reports_item ON content_reports(item_id);

            CREATE TABLE IF NOT EXISTS blocked_users (
                user_id TEXT NOT NULL,
                blocked_user_id TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (user_id, blocked_user_id)
            );
            ",
        )?;
        Ok(())
    }

    pub fn send_chat_message(
        &self,
        room_id: &str,
        user_id: &str,
        username: &str,
        content: &str,
    ) -> Result<ChatMessage, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        conn.execute(
            "INSERT INTO chat_messages (room_id, user_id, username, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![room_id, user_id, username, content, now.to_rfc3339()],
        )?;
        let id = conn.last_insert_rowid();
        Ok(ChatMessage {
            id,
            room_id: room_id.to_string(),
            user_id: user_id.to_string(),
            username: username.to_string(),
            content: content.to_string(),
            created_at: now,
        })
    }

    pub fn get_chat_messages(
        &self,
        room_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, room_id, user_id, username, content, created_at
             FROM chat_messages WHERE room_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![room_id, limit], |row| {
            let ts: String = row.get(5)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                room_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                content: row.get(4)?,
                created_at: ts.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut messages: Vec<ChatMessage> = rows.filter_map(|r| r.ok()).collect();
        messages.reverse();
        Ok(messages)
    }

    pub fn send_private_message(
        &self,
        from_id: &str,
        from_name: &str,
        to_id: &str,
        to_name: &str,
        content: &str,
    ) -> Result<PrivateMessage, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        conn.execute(
            "INSERT INTO private_messages (from_user_id, from_username, to_user_id, to_username, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![from_id, from_name, to_id, to_name, content, now.to_rfc3339()],
        )?;
        let id = conn.last_insert_rowid();
        Ok(PrivateMessage {
            id,
            from_user_id: from_id.to_string(),
            from_username: from_name.to_string(),
            to_user_id: to_id.to_string(),
            to_username: to_name.to_string(),
            content: content.to_string(),
            read: false,
            created_at: now,
        })
    }

    pub fn get_private_messages(
        &self,
        user_id: &str,
        other_user_id: &str,
        limit: usize,
    ) -> Result<Vec<PrivateMessage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, from_user_id, from_username, to_user_id, to_username, content, read, created_at
             FROM private_messages
             WHERE (from_user_id = ?1 AND to_user_id = ?2) OR (from_user_id = ?2 AND to_user_id = ?1)
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![user_id, other_user_id, limit], |row| {
            let ts: String = row.get(7)?;
            Ok(PrivateMessage {
                id: row.get(0)?,
                from_user_id: row.get(1)?,
                from_username: row.get(2)?,
                to_user_id: row.get(3)?,
                to_username: row.get(4)?,
                content: row.get(5)?,
                read: row.get(6)?,
                created_at: ts.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut messages: Vec<PrivateMessage> = rows.filter_map(|r| r.ok()).collect();
        messages.reverse();
        Ok(messages)
    }

    pub fn mark_messages_read(&self, user_id: &str, from_user_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE private_messages SET read = 1 WHERE to_user_id = ?1 AND from_user_id = ?2 AND read = 0",
            params![user_id, from_user_id],
        )?;
        Ok(())
    }

    pub fn get_conversations(&self, user_id: &str) -> Result<Vec<(String, String, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                CASE WHEN from_user_id = ?1 THEN to_user_id ELSE from_user_id END as other_id,
                CASE WHEN from_user_id = ?1 THEN to_username ELSE from_username END as other_name,
                COUNT(CASE WHEN to_user_id = ?1 AND read = 0 THEN 1 END) as unread
             FROM private_messages
             WHERE from_user_id = ?1 OR to_user_id = ?1
             GROUP BY other_id
             ORDER BY MAX(created_at) DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_report(
        &self,
        item_id: &str,
        item_name: &str,
        reporter_id: &str,
        reporter_name: &str,
        reason: &ReportReason,
        details: &str,
    ) -> Result<ContentReport, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        conn.execute(
            "INSERT INTO content_reports (item_id, item_name, reporter_id, reporter_name, reason, details, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![item_id, item_name, reporter_id, reporter_name, reason.as_str(), details, now.to_rfc3339()],
        )?;
        let id = conn.last_insert_rowid();
        Ok(ContentReport {
            id,
            item_id: item_id.to_string(),
            item_name: item_name.to_string(),
            reporter_id: reporter_id.to_string(),
            reporter_name: reporter_name.to_string(),
            reason: reason.clone(),
            details: details.to_string(),
            status: ReportStatus::Open,
            created_at: now,
        })
    }

    pub fn get_reports(&self, status: Option<&str>) -> Result<Vec<ContentReport>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let (query, param): (&str, Vec<String>) = match status {
            Some(s) => (
                "SELECT id, item_id, item_name, reporter_id, reporter_name, reason, details, status, created_at FROM content_reports WHERE status = ?1 ORDER BY created_at DESC",
                vec![s.to_string()],
            ),
            None => (
                "SELECT id, item_id, item_name, reporter_id, reporter_name, reason, details, status, created_at FROM content_reports ORDER BY created_at DESC",
                vec![],
            ),
        };
        let mut stmt = conn.prepare(query)?;
        let rows = if param.is_empty() {
            stmt.query_map([], Self::map_report)?
        } else {
            stmt.query_map(params![param[0]], Self::map_report)?
        };
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update_report_status(&self, report_id: i64, status: &ReportStatus) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE content_reports SET status = ?1 WHERE id = ?2",
            params![status.as_str(), report_id],
        )?;
        Ok(())
    }

    fn map_report(row: &rusqlite::Row) -> Result<ContentReport, rusqlite::Error> {
        let reason_str: String = row.get(5)?;
        let status_str: String = row.get(7)?;
        let ts: String = row.get(8)?;
        Ok(ContentReport {
            id: row.get(0)?,
            item_id: row.get(1)?,
            item_name: row.get(2)?,
            reporter_id: row.get(3)?,
            reporter_name: row.get(4)?,
            reason: ReportReason::from_str(&reason_str),
            details: row.get(6)?,
            status: ReportStatus::from_str(&status_str),
            created_at: ts.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn block_user(&self, user_id: &str, blocked_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO blocked_users (user_id, blocked_user_id, created_at) VALUES (?1, ?2, ?3)",
            params![user_id, blocked_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn unblock_user(&self, user_id: &str, blocked_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM blocked_users WHERE user_id = ?1 AND blocked_user_id = ?2",
            params![user_id, blocked_id],
        )?;
        Ok(())
    }

    pub fn is_blocked(&self, user_id: &str, other_id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM blocked_users WHERE user_id = ?1 AND blocked_user_id = ?2",
            params![user_id, other_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_blocked_users(&self, user_id: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT blocked_user_id FROM blocked_users WHERE user_id = ?1",
        )?;
        let rows = stmt.query_map(params![user_id], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
