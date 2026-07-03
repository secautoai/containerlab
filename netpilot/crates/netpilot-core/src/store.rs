//! On-disk lab store.
//!
//! Layout:
//! ```text
//! <data_dir>/
//!   labs/<lab_id>/lab.yaml          — the topology document
//!   labs/<lab_id>/nodes/<node_id>/  — runtime artifacts (disk overlays, configs)
//!   images/<template>/<version>/    — base images (ImageLibrary)
//!   templates/*.yaml                — user templates
//! ```

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::lab::Lab;

#[derive(Debug, Clone)]
pub struct LabStore {
    data_dir: PathBuf,
}

/// Summary line for lab listings (avoids loading full topologies).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LabSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub folder: String,
    pub node_count: usize,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

impl LabStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(data_dir.join("labs"))?;
        std::fs::create_dir_all(data_dir.join("images"))?;
        std::fs::create_dir_all(data_dir.join("templates"))?;
        Ok(Self { data_dir })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn labs_dir(&self) -> PathBuf {
        self.data_dir.join("labs")
    }

    pub fn images_dir(&self) -> PathBuf {
        self.data_dir.join("images")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.data_dir.join("templates")
    }

    pub fn lab_dir(&self, id: Uuid) -> PathBuf {
        self.labs_dir().join(id.to_string())
    }

    /// Directory holding a node's runtime artifacts (overlay disk, config media).
    pub fn node_dir(&self, lab: Uuid, node: Uuid) -> PathBuf {
        self.lab_dir(lab).join("nodes").join(node.to_string())
    }

    fn lab_file(&self, id: Uuid) -> PathBuf {
        self.lab_dir(id).join("lab.yaml")
    }

    pub fn save(&self, lab: &Lab) -> Result<()> {
        let dir = self.lab_dir(lab.id);
        std::fs::create_dir_all(&dir)?;
        let yaml = serde_yaml::to_string(lab)?;
        // Write-then-rename for crash safety.
        let tmp = dir.join("lab.yaml.tmp");
        std::fs::write(&tmp, yaml)?;
        std::fs::rename(tmp, self.lab_file(lab.id))?;
        Ok(())
    }

    pub fn load(&self, id: Uuid) -> Result<Lab> {
        let path = self.lab_file(id);
        if !path.exists() {
            return Err(CoreError::LabNotFound(id.to_string()));
        }
        let text = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&text)?)
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let dir = self.lab_dir(id);
        if !dir.exists() {
            return Err(CoreError::LabNotFound(id.to_string()));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<LabSummary>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.labs_dir())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            match self.load(id) {
                Ok(lab) => out.push(LabSummary {
                    id: lab.id,
                    name: lab.name,
                    description: lab.description,
                    folder: lab.folder,
                    node_count: lab.nodes.len(),
                    modified_at: lab.modified_at,
                }),
                Err(e) => tracing::warn!("skipping unreadable lab {id}: {e}"),
            }
        }
        out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::Lab;

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = LabStore::new(dir.path()).unwrap();

        let mut lab = Lab::new("ospf-lab");
        lab.description = "three routers".into();
        store.save(&lab).unwrap();

        let loaded = store.load(lab.id).unwrap();
        assert_eq!(loaded.name, "ospf-lab");
        assert_eq!(loaded.description, "three routers");

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, lab.id);

        store.delete(lab.id).unwrap();
        assert!(store.load(lab.id).is_err());
    }
}
