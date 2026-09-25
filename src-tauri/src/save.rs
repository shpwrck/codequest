use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

/// Serializes every save read-modify-write in this process. The guarded value
/// carries no invariant, so a panic while it was held cannot leave anything
/// inconsistent; the lock is recovered instead of failing every later save.
fn save_write_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn path_for(cartridge_path: &Path) -> PathBuf {
    let mut file_name = cartridge_path
        .file_name()
        .map_or_else(std::ffi::OsString::new, std::ffi::OsStr::to_os_string);
    file_name.push(".sav");
    cartridge_path.with_file_name(file_name)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SaveDocument {
    schema_version: u32,
    #[serde(default)]
    data: BTreeMap<String, serde_json::Value>,
}

impl Default for SaveDocument {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            data: BTreeMap::new(),
        }
    }
}

/// Reads the save at `path`. An empty file (a create that never got its first
/// write) is a fresh save. Anything else that does not parse is reported as
/// corrupt rather than read as empty: every writer starts from the document
/// read here, so reading a damaged save as empty would let the next update
/// replace all of it with a single key.
fn read_document(path: &Path) -> Result<SaveDocument, String> {
    let bytes = std::fs::read(path).map_err(|_| "COULD NOT READ CARTRIDGE SAVE".to_string())?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(SaveDocument::default());
    }
    match serde_json::from_slice::<SaveDocument>(&bytes) {
        Ok(document) if document.schema_version == SCHEMA_VERSION => Ok(document),
        Ok(_) => Err("UNSUPPORTED CARTRIDGE SAVE".to_string()),
        Err(_) => Err("CARTRIDGE SAVE IS CORRUPT".to_string()),
    }
}

/// Reads the save at `path`, creating an empty one when none exists yet.
/// Callers hold the save lock, so creation never races a concurrent writer.
fn read_or_create_document(path: &Path) -> Result<SaveDocument, String> {
    match std::fs::metadata(path) {
        Ok(_) => read_document(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let document = SaveDocument::default();
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
            serde_json::to_writer_pretty(&mut file, &document)
                .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
            file.write_all(b"\n")
                .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
            file.sync_all()
                .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
            Ok(document)
        }
        Err(_) => Err("COULD NOT READ CARTRIDGE SAVE".to_string()),
    }
}

fn persist_document(path: &Path, document: &SaveDocument) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    serde_json::to_writer_pretty(&mut temporary, document)
        .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    temporary
        .write_all(b"\n")
        .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    temporary
        .persist(path)
        .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
    Ok(())
}

/// Atomically read-modify-writes the value stored under `key`.
///
/// The whole cycle holds the save lock and starts from the document on disk,
/// so concurrent updates (a question batch landing while an answer is
/// recorded, or two batches for the same cartridge) cannot lose each other the
/// way a `get` on an old snapshot followed by `set` can. A missing value starts
/// from `T::default()`. A present value that no longer deserializes as `T` is
/// reported and left untouched rather than replaced by a default, and so is a
/// save file that does not parse at all. The save is rewritten only when
/// `change` actually altered the value.
pub(crate) fn update<T, R>(
    cartridge_path: &Path,
    key: &str,
    change: impl FnOnce(&mut T) -> R,
) -> Result<R, String>
where
    T: Serialize + DeserializeOwned + Default,
{
    let _write_guard = save_write_lock();
    let path = path_for(cartridge_path);
    let mut document = read_or_create_document(&path)?;
    let previous = document.data.get(key).cloned();
    let mut value = match &previous {
        Some(stored) => serde_json::from_value::<T>(stored.clone())
            .map_err(|_| "CARTRIDGE SAVE DATA IS UNREADABLE".to_string())?,
        None => T::default(),
    };
    let result = change(&mut value);
    let next = serde_json::to_value(&value)
        .map_err(|_| "COULD NOT SERIALIZE CARTRIDGE SAVE".to_string())?;
    if previous.as_ref() != Some(&next) {
        document.data.insert(key.to_string(), next);
        persist_document(&path, &document)?;
    }
    Ok(result)
}

/// A snapshot of one cartridge save, taken when it was opened. Game code
/// reads through a snapshot and writes through [`update`], which re-reads the
/// save under the lock.
#[derive(Clone, Debug)]
pub(crate) struct SaveFile {
    #[cfg(test)]
    path: PathBuf,
    document: SaveDocument,
}

impl SaveFile {
    pub(crate) fn open_or_create(cartridge_path: &Path) -> Result<Self, String> {
        let path = path_for(cartridge_path);
        let document = {
            let _write_guard = save_write_lock();
            read_or_create_document(&path)?
        };
        Ok(Self {
            #[cfg(test)]
            path,
            document,
        })
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.document
            .data
            .get(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    /// Replaces the value under `key`, keeping every other key as it is on
    /// disk now. Tests use it to stage saves written by other builds.
    #[cfg(test)]
    pub(crate) fn set<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), String> {
        let _write_guard = save_write_lock();
        let value = serde_json::to_value(value)
            .map_err(|_| "COULD NOT SERIALIZE CARTRIDGE SAVE".to_string())?;
        self.document = read_document(&self.path)?;
        let previous = self.document.data.insert(key.to_string(), value);
        if let Err(error) = persist_document(&self.path, &self.document) {
            if let Some(previous) = previous {
                self.document.data.insert(key.to_string(), previous);
            } else {
                self.document.data.remove(key);
            }
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_cartridge_path() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "codequest-save-test-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn save_path_appends_to_a_dotted_cartridge_name() {
        assert_eq!(
            path_for(Path::new("/games/demo.v2")),
            PathBuf::from("/games/demo.v2.sav")
        );
    }

    #[test]
    fn namespaced_data_survives_updates_from_other_game_systems() {
        let cartridge_path = temporary_cartridge_path();
        let save_path = path_for(&cartridge_path);
        let mut quest_save = SaveFile::open_or_create(&cartridge_path).unwrap();
        let mut claude_save = SaveFile::open_or_create(&cartridge_path).unwrap();

        quest_save
            .set("quest.progress", &serde_json::json!({ "bosses": 2 }))
            .unwrap();
        claude_save
            .set(
                "claude.question_batches",
                &serde_json::json!([{ "level": 1, "questions": [] }]),
            )
            .unwrap();

        let reloaded = SaveFile::open_or_create(&cartridge_path).unwrap();
        assert_eq!(
            reloaded.get::<serde_json::Value>("quest.progress"),
            Some(serde_json::json!({ "bosses": 2 }))
        );
        assert_eq!(
            reloaded.get::<serde_json::Value>("claude.question_batches"),
            Some(serde_json::json!([{ "level": 1, "questions": [] }]))
        );

        std::fs::remove_file(save_path).unwrap();
    }

    #[test]
    fn concurrent_updates_to_shared_and_separate_keys_never_lose_each_other() {
        let cartridge_path = temporary_cartridge_path();
        let writers = 6;
        let writes_per_writer = 8;

        std::thread::scope(|scope| {
            for writer in 0..writers {
                let cartridge_path = &cartridge_path;
                scope.spawn(move || {
                    for write in 0..writes_per_writer {
                        update(cartridge_path, "shared.log", |log: &mut Vec<u32>| {
                            log.push(writer * 100 + write);
                        })
                        .unwrap();
                        update(
                            cartridge_path,
                            &format!("writer.{writer}"),
                            |count: &mut u32| *count += 1,
                        )
                        .unwrap();
                    }
                });
            }
        });

        let reloaded = SaveFile::open_or_create(&cartridge_path).unwrap();
        let mut log = reloaded.get::<Vec<u32>>("shared.log").unwrap();
        log.sort_unstable();
        let mut expected = (0..writers)
            .flat_map(|writer| (0..writes_per_writer).map(move |write| writer * 100 + write))
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(log, expected);
        for writer in 0..writers {
            assert_eq!(
                reloaded.get::<u32>(&format!("writer.{writer}")),
                Some(writes_per_writer)
            );
        }

        std::fs::remove_file(path_for(&cartridge_path)).unwrap();
    }

    #[test]
    fn a_stale_snapshot_cannot_overwrite_an_update_to_another_key() {
        let cartridge_path = temporary_cartridge_path();
        let mut stale = SaveFile::open_or_create(&cartridge_path).unwrap();

        update(
            &cartridge_path,
            "quiz.progress",
            |answered: &mut Vec<String>| {
                answered.push("WHY?".into());
            },
        )
        .unwrap();
        stale.set("quest.progress", &1).unwrap();

        let reloaded = SaveFile::open_or_create(&cartridge_path).unwrap();
        assert_eq!(
            reloaded.get::<Vec<String>>("quiz.progress"),
            Some(vec!["WHY?".to_string()])
        );
        assert_eq!(reloaded.get::<u32>("quest.progress"), Some(1));

        std::fs::remove_file(path_for(&cartridge_path)).unwrap();
    }

    #[test]
    fn updates_refuse_to_replace_a_value_they_cannot_read() {
        let cartridge_path = temporary_cartridge_path();
        let mut save = SaveFile::open_or_create(&cartridge_path).unwrap();
        save.set("quiz.progress", &"FROM A NEWER BUILD").unwrap();

        let error = update(&cartridge_path, "quiz.progress", |count: &mut u32| {
            *count += 1;
        })
        .unwrap_err();

        assert_eq!(error, "CARTRIDGE SAVE DATA IS UNREADABLE");
        let reloaded = SaveFile::open_or_create(&cartridge_path).unwrap();
        assert_eq!(
            reloaded.get::<String>("quiz.progress").as_deref(),
            Some("FROM A NEWER BUILD")
        );

        std::fs::remove_file(path_for(&cartridge_path)).unwrap();
    }

    #[test]
    fn a_save_that_does_not_parse_is_refused_and_left_untouched() {
        let cartridge_path = temporary_cartridge_path();
        let save_path = path_for(&cartridge_path);
        // A save cut off mid-write or by a sync conflict: its final brace is gone.
        let truncated =
            r#"{"schema_version":1,"data":{"quiz.batches":[{"level":1}],"quiz.progress":{"a":1}}"#;
        std::fs::write(&save_path, truncated).unwrap();

        let bump = |count: &mut u32| *count += 1;

        assert_eq!(
            update(&cartridge_path, "quiz.journal", bump).unwrap_err(),
            "CARTRIDGE SAVE IS CORRUPT"
        );
        assert_eq!(
            SaveFile::open_or_create(&cartridge_path).unwrap_err(),
            "CARTRIDGE SAVE IS CORRUPT"
        );
        assert_eq!(std::fs::read_to_string(&save_path).unwrap(), truncated);

        // Valid JSON that is not a save document is refused the same way.
        std::fs::write(&save_path, "[1, 2, 3]").unwrap();
        assert_eq!(
            update(&cartridge_path, "quiz.journal", bump).unwrap_err(),
            "CARTRIDGE SAVE IS CORRUPT"
        );
        assert_eq!(std::fs::read_to_string(&save_path).unwrap(), "[1, 2, 3]");

        std::fs::remove_file(save_path).unwrap();
    }

    #[test]
    fn an_empty_save_file_starts_a_fresh_save() {
        let cartridge_path = temporary_cartridge_path();
        let save_path = path_for(&cartridge_path);
        std::fs::write(&save_path, " \n").unwrap();

        assert_eq!(
            SaveFile::open_or_create(&cartridge_path)
                .unwrap()
                .get::<u32>("quiz.progress"),
            None
        );
        update(&cartridge_path, "quiz.progress", |count: &mut u32| {
            *count = 1
        })
        .unwrap();

        let reloaded = SaveFile::open_or_create(&cartridge_path).unwrap();
        assert_eq!(reloaded.get::<u32>("quiz.progress"), Some(1));
        std::fs::remove_file(save_path).unwrap();
    }

    #[test]
    fn unchanged_updates_do_not_rewrite_the_save() {
        let cartridge_path = temporary_cartridge_path();
        update(&cartridge_path, "quiz.progress", |count: &mut u32| {
            *count = 3
        })
        .unwrap();
        // Reformat the file by hand; a rewrite would restore the canonical form.
        let compact = r#"{"schema_version":1,"data":{"quiz.progress":3}}"#;
        std::fs::write(path_for(&cartridge_path), compact).unwrap();

        update(&cartridge_path, "quiz.progress", |count: &mut u32| {
            *count = 3
        })
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(path_for(&cartridge_path)).unwrap(),
            compact
        );
        std::fs::remove_file(path_for(&cartridge_path)).unwrap();
    }
}
