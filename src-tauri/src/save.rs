use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

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

fn read_document(path: &Path) -> Result<SaveDocument, String> {
    let bytes = std::fs::read(path).map_err(|_| "COULD NOT READ CARTRIDGE SAVE".to_string())?;
    match serde_json::from_slice::<SaveDocument>(&bytes) {
        Ok(document) if document.schema_version == SCHEMA_VERSION => Ok(document),
        Ok(_) => Err("UNSUPPORTED CARTRIDGE SAVE".to_string()),
        Err(_) => Ok(SaveDocument::default()),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SaveFile {
    path: PathBuf,
    document: SaveDocument,
}

impl SaveFile {
    pub(crate) fn open_or_create(cartridge_path: &Path) -> Result<Self, String> {
        let path = path_for(cartridge_path);
        match std::fs::read(&path) {
            Ok(_) => {
                let document = read_document(&path)?;
                Ok(Self { path, document })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let document = SaveDocument::default();
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
                serde_json::to_writer_pretty(&mut file, &document)
                    .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
                file.write_all(b"\n")
                    .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
                file.sync_all()
                    .map_err(|_| "COULD NOT CREATE CARTRIDGE SAVE".to_string())?;
                Ok(Self { path, document })
            }
            Err(_) => Err("COULD NOT READ CARTRIDGE SAVE".to_string()),
        }
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.document
            .data
            .get(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    pub(crate) fn set<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), String> {
        let value = serde_json::to_value(value)
            .map_err(|_| "COULD NOT SERIALIZE CARTRIDGE SAVE".to_string())?;
        self.document = read_document(&self.path)?;
        let previous = self.document.data.insert(key.to_string(), value);
        if let Err(error) = self.persist() {
            if let Some(previous) = previous {
                self.document.data.insert(key.to_string(), previous);
            } else {
                self.document.data.remove(key);
            }
            return Err(error);
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
        serde_json::to_writer_pretty(&mut temporary, &self.document)
            .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
        temporary
            .write_all(b"\n")
            .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
        temporary
            .persist(&self.path)
            .map_err(|_| "COULD NOT WRITE CARTRIDGE SAVE".to_string())?;
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
}
