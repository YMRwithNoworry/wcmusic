use std::path::Path;

use rusqlite::{Connection, params};

use crate::{CoreError, Playlist, Track};

pub struct MusicDatabase {
    connection: Connection,
}

impl MusicDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let connection = Connection::open(path)?;
        let database = Self { connection };
        database.migrate()?;
        Ok(database)
    }

    pub fn in_memory() -> Result<Self, CoreError> {
        let connection = Connection::open_in_memory()?;
        let database = Self { connection };
        database.migrate()?;
        Ok(database)
    }

    pub fn upsert_track(&self, track: &Track) -> Result<(), CoreError> {
        self.connection.execute(
            "INSERT INTO tracks (id, title, artist, album, duration_ms, uri, artwork_uri, source, source_id, quality)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, artist=excluded.artist,
             album=excluded.album, duration_ms=excluded.duration_ms, uri=excluded.uri,
             artwork_uri=excluded.artwork_uri, source=excluded.source,
             source_id=excluded.source_id, quality=excluded.quality",
            params![
                track.id,
                track.title,
                track.artist,
                track.album,
                track.duration_ms,
                track.uri,
                track.artwork_uri,
                serde_json::to_string(&track.source)?,
                track.source_id,
                track.quality,
            ],
        )?;
        Ok(())
    }

    pub fn save_playlist(&mut self, playlist: &Playlist) -> Result<(), CoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO playlists (id, name, description) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description",
            params![playlist.id, playlist.name, playlist.description],
        )?;
        transaction.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            [&playlist.id],
        )?;
        for (position, track) in playlist.tracks.iter().enumerate() {
            transaction.execute(
                "INSERT OR REPLACE INTO tracks (id, title, artist, album, duration_ms, uri, artwork_uri, source, source_id, quality)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![track.id, track.title, track.artist, track.album, track.duration_ms, track.uri,
                    track.artwork_uri, serde_json::to_string(&track.source)?, track.source_id, track.quality],
            )?;
            transaction.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist.id, track.id, position],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn track_count(&self) -> Result<u64, CoreError> {
        Ok(self
            .connection
            .query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?)
    }

    fn migrate(&self) -> Result<(), CoreError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                uri TEXT NOT NULL,
                artwork_uri TEXT,
                source TEXT NOT NULL,
                source_id TEXT,
                quality TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title);
             CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
             CREATE TABLE IF NOT EXISTS playlists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                PRIMARY KEY (playlist_id, position)
             );",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_upserts_tracks() {
        let database = MusicDatabase::in_memory().unwrap();
        database
            .upsert_track(&Track::local("one", "Seedling", "file:///seedling.flac"))
            .unwrap();
        assert_eq!(database.track_count().unwrap(), 1);
    }
}
