use rusqlite::{Connection, params};
use std::sync::Mutex;

/// Info about a previously recorded link.
pub struct PriorLink {
    pub author_id: u64,
    pub author_name: String,
    pub timestamp: Option<i64>,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS links (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                guild_id    INTEGER NOT NULL,
                channel_id  INTEGER NOT NULL,
                author_id   INTEGER NOT NULL,
                author_name TEXT    NOT NULL,
                message_id  INTEGER NOT NULL,
                url         TEXT    NOT NULL,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE INDEX IF NOT EXISTS idx_links_lookup
                ON links (guild_id, channel_id, url);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Look up whether this normalized URL has been posted in this channel before.
    /// Returns the first match (oldest).
    pub fn find_duplicate(
        &self,
        guild_id: u64,
        channel_id: u64,
        url: &str,
    ) -> rusqlite::Result<Option<PriorLink>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT author_id, author_name, created_at
             FROM links
             WHERE guild_id = ?1 AND channel_id = ?2 AND url = ?3
             ORDER BY created_at ASC
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![guild_id as i64, channel_id as i64, url])?;

        if let Some(row) = rows.next()? {
            let author_id_raw: i64 = row.get(0)?;
            Ok(Some(PriorLink {
                author_id: author_id_raw as u64,
                author_name: row.get(1)?,
                timestamp: row.get(2)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Record a link posted in a channel.
    pub fn record_link(
        &self,
        guild_id: u64,
        channel_id: u64,
        author_id: u64,
        author_name: &str,
        message_id: u64,
        url: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO links (guild_id, channel_id, author_id, author_name, message_id, url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                guild_id as i64,
                channel_id as i64,
                author_id as i64,
                author_name,
                message_id as i64,
                url,
            ],
        )?;
        Ok(())
    }
}
