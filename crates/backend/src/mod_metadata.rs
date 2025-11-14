use std::{
    collections::HashMap, fs::File, io::{Cursor, Read}, path::{Path, PathBuf}, sync::Arc
};

use bridge::instance::ModSummary;
use image::imageops::FilterType;
use rustc_hash::FxHashMap;
use schema::content::ContentSource;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeAs, SerializeAs};
use sha1::{Digest, Sha1};
use std::sync::RwLock;
use zip::read::ZipFile;

pub struct ModMetadataManager {
    _content_meta_dir: Arc<Path>,
    sources_json: PathBuf,
    by_hash: RwLock<FxHashMap<[u8; 20], Option<Arc<ModSummary>>>>,
    content_sources: RwLock<FxHashMap<[u8; 20], ContentSource>>,
}

impl ModMetadataManager {
    pub fn load(content_meta_dir: Arc<Path>) -> Self {
        let sources_json = content_meta_dir.join("sources.json");

        let content_sources = if let Ok(data) = std::fs::read(&sources_json) {
            let content_sources = serde_json::from_slice(&data);
            content_sources.map(|v: DeserializedContentSources| v.0).unwrap_or_default()
        } else {
            Default::default()
        };

        Self {
            _content_meta_dir: content_meta_dir,
            sources_json,
            by_hash: Default::default(),
            content_sources: RwLock::new(content_sources),
        }
    }

    pub fn set_content_sources(&self, sources: impl Iterator<Item = ([u8; 20], ContentSource)>) {
        let mut content_sources = self.content_sources.write().unwrap();

        let mut changed = false;
        for (hash, source) in sources {
            let old = content_sources.insert(hash, source);
            if old != Some(source) {
                changed = true;
            }
        }

        if changed {
            let serialized = SerializedContentSources(&content_sources);
            if let Ok(content) = serde_json::to_vec(&serialized) {
                let _ = crate::write_safe(&self.sources_json, &content);
            }
        }
    }

    pub fn get(&self, file: &mut std::fs::File) -> Option<Arc<ModSummary>> {
        let mut hasher = Sha1::new();
        let _ = std::io::copy(file, &mut hasher).ok()?;
        let actual_hash: [u8; 20] = hasher.finalize().into();

        // todo: cache on disk?

        if let Some(summary) = self.by_hash.read().unwrap().get(&actual_hash) {
            return summary.clone();
        }

        let summary = Self::load_mod_summary(file);

        self.by_hash.write().unwrap().insert(actual_hash, summary.clone());

        summary
    }

    fn load_mod_summary(file: &mut std::fs::File) -> Option<Arc<ModSummary>> {
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let file = match archive.by_name("fabric.mod.json") {
            Ok(file) => file,
            Err(..) => {
                return None;
            },
        };

        let fabric_mod_json: FabricModJson = serde_json::from_reader(file).unwrap();

        let name = fabric_mod_json.name.unwrap_or_else(|| Arc::clone(&fabric_mod_json.id));

        let icon = match fabric_mod_json.icon {
            Some(icon) => match icon {
                Icon::Single(icon) => Some(icon),
                Icon::Sizes(hash_map) => {
                    const DESIRED_SIZE: usize = 64;
                    hash_map.iter().min_by_key(|size| size.0.abs_diff(DESIRED_SIZE)).map(|e| Arc::clone(e.1))
                },
            },
            None => None,
        };

        let mut png_icon: Option<Arc<[u8]>> = None;
        if let Some(icon) = icon && let Ok(icon_file) = archive.by_name(&icon) {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = fabric_mod_json.authors && !authors.is_empty() {
            let mut authors_string = "By ".to_owned();
            let mut first = true;
            for author in authors {
                if first {
                    first = false;
                } else {
                    authors_string.push_str(", ");
                }
                authors_string.push_str(author.name());
            }
            authors_string.into()
        } else {
            "".into()
        };

        Some(Arc::new(ModSummary {
            id: fabric_mod_json.id,
            name,
            authors,
            version_str: format!("v{}", fabric_mod_json.version).into(),
            png_icon,
        }))
    }
}

fn load_icon(mut icon_file: ZipFile<'_, &mut File>) -> Option<Arc<[u8]>> {
    let mut icon_bytes = Vec::with_capacity(icon_file.size() as usize);
    let Ok(_) = icon_file.read_to_end(&mut icon_bytes) else {
        return None;
    };

    let Ok(image) = image::load_from_memory(&icon_bytes) else {
        return None;
    };

    let width = image.width();
    let height = image.height();
    if image.width() != 64 || image.height() != 64 {
        let filter = if width > 64 || height > 64 {
            FilterType::Lanczos3
        } else {
            FilterType::Nearest
        };
        let resized = image.resize_exact(64, 64, filter);

        icon_bytes.clear();
        let mut cursor = Cursor::new(&mut icon_bytes);
        if resized.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
            return None;
        }
    }

    Some(icon_bytes.into())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FabricModJson {
    id: Arc<str>,
    version: Arc<str>,
    name: Option<Arc<str>>,
    // description: Option<Arc<str>>,
    authors: Option<Vec<Person>>,
    icon: Option<Icon>,
    // #[serde(alias = "requires")]
    // depends: Option<HashMap<Arc<str>, Dependency>>,
    // breaks: Option<HashMap<Arc<str>, Dependency>>,
}

// #[derive(Deserialize, Debug)]
// #[serde(untagged)]
// enum Dependency {
//     Single(Arc<str>),
//     Multiple(Vec<Arc<str>>)
// }

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Icon {
    Single(Arc<str>),
    Sizes(HashMap<usize, Arc<str>>),
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Person {
    Name(Arc<str>),
    NameAndContact { name: Arc<str> },
}

impl Person {
    pub fn name(&self) -> &str {
        match self {
            Person::Name(name) => name,
            Person::NameAndContact { name, .. } => name,
        }
    }
}

#[serde_as]
#[derive(Serialize)]
struct SerializedContentSources<'a>(
    #[serde_as(as = "FxHashMap<SerializeAsHex, _>")]
    &'a FxHashMap<[u8; 20], ContentSource>
);

struct SerializeAsHex {}

impl SerializeAs<[u8; 20]> for SerializeAsHex {
    fn serialize_as<S>(source: &[u8; 20], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        hex::serde::serialize(source, serializer)
    }
}

#[serde_as]
#[derive(Deserialize)]
struct DeserializedContentSources(
    #[serde_as(as = "FxHashMap<DeserializeAsHex, _>")]
    FxHashMap<[u8; 20], ContentSource>
);

struct DeserializeAsHex {}

impl<'de> DeserializeAs<'de, [u8; 20]> for DeserializeAsHex {
    fn deserialize_as<D>(deserializer: D) -> Result<[u8; 20], D::Error>
    where
        D: serde::Deserializer<'de> {
        hex::serde::deserialize(deserializer)
    }
}
