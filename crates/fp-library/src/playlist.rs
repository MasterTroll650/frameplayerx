//! Playlists: ordered lists of videos from any source. What locked sources
//! hide ([`Library::set_hidden`]) is left out of them as everywhere else.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};
use crate::library::{Hidden, Library, ensure_media, get_many};
use crate::record::{MediaId, MediaRecord, PlaylistId, now};

/// A playlist summary.
#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Videos in it, those hidden left out.
    pub item_count: usize,
    /// Its first video (hidden ones left out), for a picture.
    pub first: Option<MediaRecord>,
}

/// A video in a playlist, at `position` among all its entries (hidden ones
/// counted), which is what edits take.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistEntry {
    pub position: usize,
    pub record: MediaRecord,
}

fn check_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        Err(Error::InvalidArgument("playlist name is empty".into()))
    } else {
        Ok(name)
    }
}

fn ids(conn: &Connection, id: PlaylistId) -> Result<Vec<MediaId>> {
    let mut stmt = conn.prepare_cached(
        "SELECT media_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position",
    )?;
    let rows = stmt.query_map([id.0], |r| Ok(MediaId(r.get(0)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn write_ids(conn: &Connection, id: PlaylistId, items: &[MediaId]) -> Result<()> {
    conn.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id.0])?;
    let mut ins = conn.prepare_cached(
        "INSERT INTO playlist_items (playlist_id, position, media_id) VALUES (?1, ?2, ?3)",
    )?;
    for (pos, m) in items.iter().enumerate() {
        ensure_media(conn, *m)?;
        ins.execute(params![id.0, pos as i64, m.0])?;
    }
    Ok(())
}

fn entries(conn: &Connection, id: PlaylistId, hidden: &Hidden) -> Result<Vec<PlaylistEntry>> {
    let ids = ids(conn, id)?;
    let by_id: std::collections::HashMap<MediaId, MediaRecord> = get_many(conn, &ids)?
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    Ok(ids
        .iter()
        .enumerate()
        .filter_map(|(position, m)| {
            let record = by_id.get(m)?;
            (!hidden.hides(&record.source_id, &record.location)).then(|| PlaylistEntry {
                position,
                record: record.clone(),
            })
        })
        .collect())
}

fn playlist_conn(conn: &Connection, id: PlaylistId, hidden: &Hidden) -> Result<Option<Playlist>> {
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT name, created_at FROM playlists WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((name, created_at)) = row else {
        return Ok(None);
    };
    let items = entries(conn, id, hidden)?;
    Ok(Some(Playlist {
        id,
        name,
        created_at,
        item_count: items.len(),
        first: items.into_iter().next().map(|e| e.record),
    }))
}

fn require(conn: &Connection, id: PlaylistId) -> Result<()> {
    let found: Option<i64> = conn
        .query_row("SELECT 1 FROM playlists WHERE id = ?1", [id.0], |r| {
            r.get(0)
        })
        .optional()?;
    found.map(|_| ()).ok_or(Error::PlaylistNotFound(id))
}

impl Library {
    /// Creates an empty playlist.
    pub fn create_playlist(&self, name: &str) -> Result<PlaylistId> {
        let name = check_name(name)?;
        self.with(|c| {
            c.execute(
                "INSERT INTO playlists (name, created_at) VALUES (?1, ?2)",
                params![name, now()],
            )?;
            Ok(PlaylistId(c.last_insert_rowid()))
        })
    }

    /// All playlists, by name.
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let hidden = self.hidden();
        self.with(|c| {
            let ids: Vec<PlaylistId> = {
                let mut stmt =
                    c.prepare_cached("SELECT id FROM playlists ORDER BY name COLLATE NOCASE, id")?;
                let rows = stmt.query_map([], |r| Ok(PlaylistId(r.get(0)?)))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                out.extend(playlist_conn(c, id, &hidden)?);
            }
            Ok(out)
        })
    }

    pub fn playlist(&self, id: PlaylistId) -> Result<Option<Playlist>> {
        let hidden = self.hidden();
        self.with(|c| playlist_conn(c, id, &hidden))
    }

    pub fn rename_playlist(&self, id: PlaylistId, name: &str) -> Result<()> {
        let name = check_name(name)?;
        self.with(|c| {
            let n = c.execute(
                "UPDATE playlists SET name = ?2 WHERE id = ?1",
                params![id.0, name],
            )?;
            if n == 0 {
                return Err(Error::PlaylistNotFound(id));
            }
            Ok(())
        })
    }

    /// Deletes a playlist (the videos stay).
    pub fn delete_playlist(&self, id: PlaylistId) -> Result<()> {
        self.with(|c| {
            if c.execute("DELETE FROM playlists WHERE id = ?1", [id.0])? == 0 {
                return Err(Error::PlaylistNotFound(id));
            }
            Ok(())
        })
    }

    /// The videos of a playlist in order, those hidden left out.
    pub fn playlist_items(&self, id: PlaylistId) -> Result<Vec<PlaylistEntry>> {
        let hidden = self.hidden();
        self.with(|c| {
            require(c, id)?;
            entries(c, id, &hidden)
        })
    }

    /// Appends videos to a playlist (duplicates allowed).
    pub fn add_to_playlist(&self, id: PlaylistId, media: &[MediaId]) -> Result<()> {
        self.edit_items(id, |items| {
            items.extend_from_slice(media);
            Ok(())
        })
    }

    /// Removes the entry at `position`.
    pub fn remove_from_playlist(&self, id: PlaylistId, position: usize) -> Result<()> {
        self.edit_items(id, |items| {
            if position >= items.len() {
                return Err(Error::InvalidArgument(format!(
                    "position {position} outside playlist of {}",
                    items.len()
                )));
            }
            items.remove(position);
            Ok(())
        })
    }

    /// Moves the entry at `from` so it ends up at `to`.
    pub fn move_playlist_item(&self, id: PlaylistId, from: usize, to: usize) -> Result<()> {
        self.edit_items(id, |items| {
            if from >= items.len() || to >= items.len() {
                return Err(Error::InvalidArgument(format!(
                    "move {from} -> {to} outside playlist of {}",
                    items.len()
                )));
            }
            let m = items.remove(from);
            items.insert(to, m);
            Ok(())
        })
    }

    fn edit_items(
        &self,
        id: PlaylistId,
        f: impl FnOnce(&mut Vec<MediaId>) -> Result<()>,
    ) -> Result<()> {
        self.in_tx(|c| {
            require(c, id)?;
            let mut items = ids(c, id)?;
            f(&mut items)?;
            write_ids(c, id, &items)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{HiddenRoot, MediaUpsert};

    fn setup() -> (Library, Vec<MediaId>) {
        let lib = Library::open_in_memory().unwrap();
        let ids = ["a", "b", "c", "d"]
            .iter()
            .map(|n| {
                lib.upsert(&MediaUpsert::new(format!("/v/{n}.mp4"), "movies"))
                    .unwrap()
                    .id
            })
            .collect();
        (lib, ids)
    }

    fn items(lib: &Library, p: PlaylistId) -> Vec<MediaId> {
        lib.playlist_items(p)
            .unwrap()
            .iter()
            .map(|e| e.record.id)
            .collect()
    }

    #[test]
    fn playlists_are_made_edited_and_deleted() {
        let (lib, m) = setup();
        let p = lib.create_playlist("  Evening ").unwrap();
        assert!(lib.create_playlist("   ").is_err());
        lib.add_to_playlist(p, &[m[0], m[1]]).unwrap();
        lib.add_to_playlist(p, &[m[2], m[3]]).unwrap();
        assert_eq!(items(&lib, p), vec![m[0], m[1], m[2], m[3]]);
        lib.move_playlist_item(p, 3, 0).unwrap();
        assert_eq!(items(&lib, p), vec![m[3], m[0], m[1], m[2]]);
        lib.move_playlist_item(p, 0, 3).unwrap();
        assert_eq!(items(&lib, p), vec![m[0], m[1], m[2], m[3]]);
        lib.remove_from_playlist(p, 1).unwrap();
        assert_eq!(items(&lib, p), vec![m[0], m[2], m[3]]);
        assert!(lib.remove_from_playlist(p, 9).is_err());
        assert!(lib.move_playlist_item(p, 0, 9).is_err());
        assert!(lib.add_to_playlist(p, &[MediaId(999)]).is_err());
        assert_eq!(items(&lib, p), vec![m[0], m[2], m[3]], "failed edit undone");

        let pl = lib.playlist(p).unwrap().unwrap();
        assert_eq!((pl.name.as_str(), pl.item_count), ("Evening", 3));
        assert_eq!(pl.first.map(|r| r.id), Some(m[0]));
        lib.rename_playlist(p, "Night").unwrap();
        let q = lib.create_playlist("aardvark").unwrap();
        let names: Vec<String> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["aardvark", "Night"]);

        // Removing a video removes its entries.
        lib.remove_media(m[2]).unwrap();
        assert_eq!(items(&lib, p), vec![m[0], m[3]]);
        lib.delete_playlist(q).unwrap();
        assert!(matches!(
            lib.delete_playlist(q),
            Err(Error::PlaylistNotFound(_))
        ));
        assert!(lib.playlist(q).unwrap().is_none());
    }

    #[test]
    fn locked_sources_are_left_out_and_edits_still_land() {
        let (lib, m) = setup();
        let nas = lib
            .upsert(&MediaUpsert::new("smb://nas/v/x.mp4", "nas"))
            .unwrap()
            .id;
        let p = lib.create_playlist("Mix").unwrap();
        lib.add_to_playlist(p, &[m[0], nas, m[1]]).unwrap();
        lib.set_hidden(Hidden {
            sources: vec!["nas".into()],
            ..Default::default()
        });
        let shown = lib.playlist_items(p).unwrap();
        let at: Vec<(usize, MediaId)> = shown.iter().map(|e| (e.position, e.record.id)).collect();
        assert_eq!(at, vec![(0, m[0]), (2, m[1])]);
        assert_eq!(lib.playlist(p).unwrap().unwrap().item_count, 2);
        // Removing the second one shown removes it, not the hidden one.
        lib.remove_from_playlist(p, shown[1].position).unwrap();
        lib.set_hidden(Hidden::default());
        assert_eq!(items(&lib, p), vec![m[0], nas]);

        // Hidden by where it is, too.
        lib.set_hidden(Hidden {
            locations: vec![HiddenRoot::new("smb://nas/v")],
            ..Default::default()
        });
        assert_eq!(items(&lib, p), vec![m[0]]);
        let first = lib.playlists().unwrap()[0].first.clone().map(|r| r.id);
        assert_eq!(first, Some(m[0]));
    }
}
