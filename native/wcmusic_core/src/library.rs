use std::collections::HashMap;

use parking_lot::RwLock;

use crate::Track;

#[derive(Default)]
pub struct LibraryIndex {
    tracks: RwLock<HashMap<String, Track>>,
}

impl LibraryIndex {
    pub fn replace(&self, tracks: impl IntoIterator<Item = Track>) {
        let mut index = self.tracks.write();
        index.clear();
        index.extend(tracks.into_iter().map(|track| (track.id.clone(), track)));
    }

    pub fn upsert(&self, track: Track) {
        self.tracks.write().insert(track.id.clone(), track);
    }

    pub fn remove(&self, id: &str) -> Option<Track> {
        self.tracks.write().remove(id)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Track> {
        let query = query.trim().to_lowercase();
        let tracks = self.tracks.read();
        let mut results: Vec<_> = tracks
            .values()
            .filter(|track| {
                query.is_empty()
                    || track.title.to_lowercase().contains(&query)
                    || track.artist.to_lowercase().contains(&query)
                    || track.album.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        results.sort_by_cached_key(|track| track.title.to_lowercase());
        results.truncate(limit);
        results
    }

    pub fn len(&self) -> usize {
        self.tracks.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_across_track_metadata() {
        let index = LibraryIndex::default();
        let mut track = Track::local("1", "Morning Tide", "file:///morning.flac");
        track.artist = "Greenhouse".into();
        index.upsert(track);

        assert_eq!(index.search("green", 10)[0].id, "1");
        assert!(index.search("night", 10).is_empty());
    }
}
