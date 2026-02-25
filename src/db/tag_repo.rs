use crate::db::connection::Database;
use crate::domain::models::Tag;
use crate::error::{AppError, Result};

/// Repository for [`Tag`] CRUD operations.
pub struct TagRepo<'a> {
    db: &'a Database,
}

impl<'a> TagRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new tag and return its generated id.
    pub fn create(&self, tag: &Tag) -> Result<i64> {
        self.db.conn().execute(
            "INSERT INTO tags (name, parent_id, icon) VALUES (?1, ?2, ?3)",
            rusqlite::params![tag.name, tag.parent_id, tag.icon],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Fetch a single tag by id, or return `NotFound`.
    pub fn get_by_id(&self, id: i64) -> Result<Tag> {
        self.db
            .conn()
            .query_row(
                "SELECT id, name, parent_id, icon FROM tags WHERE id = ?1",
                rusqlite::params![id],
                row_to_tag,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("Tag with id {id}"))
                }
                other => AppError::Database(other),
            })
    }

    /// Return every tag ordered by name.
    pub fn get_all(&self) -> Result<Vec<Tag>> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, name, parent_id, icon FROM tags ORDER BY name")?;
        let tags = stmt
            .query_map([], row_to_tag)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Return children of a given parent tag.
    pub fn get_children(&self, parent_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, parent_id, icon FROM tags WHERE parent_id = ?1 ORDER BY name",
        )?;
        let tags = stmt
            .query_map(rusqlite::params![parent_id], row_to_tag)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Return all root-level tags (those without a parent).
    pub fn get_root_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, parent_id, icon FROM tags WHERE parent_id IS NULL ORDER BY name",
        )?;
        let tags = stmt
            .query_map([], row_to_tag)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Update an existing tag. The tag's `id` must be `Some`.
    pub fn update(&self, tag: &Tag) -> Result<()> {
        let id = tag
            .id
            .ok_or_else(|| AppError::Validation("Cannot update a tag without an id.".into()))?;
        let affected = self.db.conn().execute(
            "UPDATE tags SET name = ?1, parent_id = ?2, icon = ?3 WHERE id = ?4",
            rusqlite::params![tag.name, tag.parent_id, tag.icon, id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Tag with id {id}")));
        }
        Ok(())
    }

    /// Delete a tag by id.
    pub fn delete(&self, id: i64) -> Result<()> {
        let affected = self
            .db
            .conn()
            .execute("DELETE FROM tags WHERE id = ?1", rusqlite::params![id])?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Tag with id {id}")));
        }
        Ok(())
    }

    /// Find a tag by its exact name.
    pub fn find_by_name(&self, name: &str) -> Result<Option<Tag>> {
        let result = self.db.conn().query_row(
            "SELECT id, name, parent_id, icon FROM tags WHERE name = ?1",
            rusqlite::params![name],
            row_to_tag,
        );
        match result {
            Ok(tag) => Ok(Some(tag)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e)),
        }
    }

    /// Seed the default tags into an empty database.
    ///
    /// Only inserts when the tags table contains zero rows so that user
    /// customisations are never overwritten.
    pub fn seed_defaults(&self, tags: &[String]) -> Result<()> {
        let count: i64 = self
            .db
            .conn()
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;

        if count > 0 {
            return Ok(());
        }

        let mut stmt = self
            .db
            .conn()
            .prepare("INSERT INTO tags (name) VALUES (?1)")?;

        for tag_name in tags {
            stmt.execute(rusqlite::params![tag_name])?;
        }

        Ok(())
    }
}

/// Map a row from the `tags` table to a [`Tag`].
fn row_to_tag(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        icon: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn create_and_get() {
        let db = setup();
        let repo = TagRepo::new(&db);

        let tag = Tag {
            id: None,
            name: "Comida".into(),
            parent_id: None,
            icon: None,
        };
        let id = repo.create(&tag).unwrap();
        assert!(id > 0);

        let fetched = repo.get_by_id(id).unwrap();
        assert_eq!(fetched.name, "Comida");
        assert_eq!(fetched.id, Some(id));
    }

    #[test]
    fn get_all() {
        let db = setup();
        let repo = TagRepo::new(&db);
        repo.seed_defaults(&["A".into(), "B".into(), "C".into()])
            .unwrap();

        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn find_by_name() {
        let db = setup();
        let repo = TagRepo::new(&db);
        repo.create(&Tag {
            id: None,
            name: "Salud".into(),
            parent_id: None,
            icon: None,
        })
        .unwrap();

        assert!(repo.find_by_name("Salud").unwrap().is_some());
        assert!(repo.find_by_name("Nope").unwrap().is_none());
    }

    #[test]
    fn update_and_delete() {
        let db = setup();
        let repo = TagRepo::new(&db);

        let id = repo
            .create(&Tag {
                id: None,
                name: "Old".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();

        repo.update(&Tag {
            id: Some(id),
            name: "New".into(),
            parent_id: None,
            icon: Some("🏷".into()),
        })
        .unwrap();

        let updated = repo.get_by_id(id).unwrap();
        assert_eq!(updated.name, "New");

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).is_err());
    }

    #[test]
    fn seed_defaults_only_once() {
        let db = setup();
        let repo = TagRepo::new(&db);

        repo.seed_defaults(&["A".into(), "B".into()]).unwrap();
        assert_eq!(repo.get_all().unwrap().len(), 2);

        // Calling again should be a no-op.
        repo.seed_defaults(&["C".into(), "D".into()]).unwrap();
        assert_eq!(repo.get_all().unwrap().len(), 2);
    }

    #[test]
    fn hierarchical_tags() {
        let db = setup();
        let repo = TagRepo::new(&db);

        let parent_id = repo
            .create(&Tag {
                id: None,
                name: "Comida".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();

        repo.create(&Tag {
            id: None,
            name: "Restaurante".into(),
            parent_id: Some(parent_id),
            icon: None,
        })
        .unwrap();

        repo.create(&Tag {
            id: None,
            name: "Supermercado".into(),
            parent_id: Some(parent_id),
            icon: None,
        })
        .unwrap();

        let children = repo.get_children(parent_id).unwrap();
        assert_eq!(children.len(), 2);

        let roots = repo.get_root_tags().unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Comida");
    }
}
