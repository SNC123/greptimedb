// Copyright 2023 Greptime Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Structures to describe metadata of files.

use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use common_time::Timestamp;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use snafu::{ResultExt, Snafu};
use store_api::storage::RegionId;
use uuid::Uuid;

use crate::sst::file_purger::{FilePurgerRef, PurgeRequest};
use crate::sst::location;

/// Type to store SST level.
pub type Level = u8;
/// Maximum level of SSTs.
pub const MAX_LEVEL: Level = 2;

#[derive(Debug, Snafu, PartialEq)]
pub struct ParseIdError {
    source: uuid::Error,
}

/// Unique id for [SST File].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct FileId(Uuid);

impl FileId {
    /// Returns a new unique [FileId] randomly.
    pub fn random() -> FileId {
        FileId(Uuid::new_v4())
    }

    /// Parses id from string.
    pub fn parse_str(input: &str) -> std::result::Result<FileId, ParseIdError> {
        Uuid::parse_str(input).map(FileId).context(ParseIdSnafu)
    }

    /// Append `.parquet` to file id to make a complete file name
    pub fn as_parquet(&self) -> String {
        format!("{}{}", self, ".parquet")
    }

    /// Append `.puffin` to file id to make a complete file name
    pub fn as_puffin(&self) -> String {
        format!("{}{}", self, ".puffin")
    }

    /// Converts [FileId] as byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<FileId> for Uuid {
    fn from(value: FileId) -> Self {
        value.0
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for FileId {
    type Err = ParseIdError;

    fn from_str(s: &str) -> std::result::Result<FileId, ParseIdError> {
        FileId::parse_str(s)
    }
}

/// Time range (min and max timestamps) of a SST file.
/// Both min and max are inclusive.
pub type FileTimeRange = (Timestamp, Timestamp);

/// Checks if two inclusive timestamp ranges overlap with each other.
pub(crate) fn overlaps(l: &FileTimeRange, r: &FileTimeRange) -> bool {
    let (l, r) = if l.0 <= r.0 { (l, r) } else { (r, l) };
    let (_, l_end) = l;
    let (r_start, _) = r;

    r_start <= l_end
}

/// Metadata of a SST file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FileMeta {
    /// Region of file.
    pub region_id: RegionId,
    /// Compared to normal file names, FileId ignore the extension
    pub file_id: FileId,
    /// Timestamp range of file. The timestamps have the same time unit as the
    /// data in the SST.
    pub time_range: FileTimeRange,
    /// SST level of the file.
    pub level: Level,
    /// Size of the file.
    pub file_size: u64,
    /// Available indexes of the file.
    pub available_indexes: SmallVec<[IndexType; 4]>,
    /// Size of the index file.
    pub index_file_size: u64,
    /// Number of rows in the file.
    ///
    /// For historical reasons, this field might be missing in old files. Thus
    /// the default value `0` doesn't means the file doesn't contains any rows,
    /// but instead means the number of rows is unknown.
    pub num_rows: u64,
    /// Number of row groups in the file.
    ///
    /// For historical reasons, this field might be missing in old files. Thus
    /// the default value `0` doesn't means the file doesn't contains any rows,
    /// but instead means the number of rows is unknown.
    pub num_row_groups: u64,
    /// Sequence in this file.
    ///
    /// This sequence is the only sequence in this file. And it's retrieved from the max
    /// sequence of the rows on generating this file.
    pub sequence: Option<NonZeroU64>,
}

/// Type of index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IndexType {
    /// Inverted index.
    InvertedIndex,
    /// Full-text index.
    FulltextIndex,
    /// Bloom Filter index
    BloomFilterIndex,
}

impl FileMeta {
    pub fn exists_index(&self) -> bool {
        !self.available_indexes.is_empty()
    }

    /// Returns true if the file has an inverted index
    pub fn inverted_index_available(&self) -> bool {
        self.available_indexes.contains(&IndexType::InvertedIndex)
    }

    /// Returns true if the file has a fulltext index
    pub fn fulltext_index_available(&self) -> bool {
        self.available_indexes.contains(&IndexType::FulltextIndex)
    }

    /// Returns true if the file has a bloom filter index.
    pub fn bloom_filter_index_available(&self) -> bool {
        self.available_indexes
            .contains(&IndexType::BloomFilterIndex)
    }

    pub fn index_file_size(&self) -> u64 {
        self.index_file_size
    }
}

/// Handle to a SST file.
#[derive(Clone)]
pub struct FileHandle {
    inner: Arc<FileHandleInner>,
}

impl fmt::Debug for FileHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileHandle")
            .field("region_id", &self.inner.meta.region_id)
            .field("file_index", &self.inner.meta.available_indexes)
            .field("index_file_size",&self.inner.meta.index_file_size)
            .field("file_id", &self.inner.meta.file_id)
            .field("time_range", &self.inner.meta.time_range)
            .field("size", &self.inner.meta.file_size)
            .field("level", &self.inner.meta.level)
            .field("compacting", &self.inner.compacting)
            .field("deleted", &self.inner.deleted)
            .finish()
    }
}

impl FileHandle {
    pub fn new(meta: FileMeta, file_purger: FilePurgerRef) -> FileHandle {
        FileHandle {
            inner: Arc::new(FileHandleInner::new(meta, file_purger)),
        }
    }

    /// Returns the region id of the file.
    pub fn region_id(&self) -> RegionId {
        self.inner.meta.region_id
    }

    /// Returns the file id.
    pub fn file_id(&self) -> FileId {
        self.inner.meta.file_id
    }

    /// Returns the complete file path of the file.
    pub fn file_path(&self, file_dir: &str) -> String {
        location::sst_file_path(file_dir, self.file_id())
    }

    /// Returns the time range of the file.
    pub fn time_range(&self) -> FileTimeRange {
        self.inner.meta.time_range
    }

    /// Mark the file as deleted and will delete it on drop asynchronously
    pub fn mark_deleted(&self) {
        self.inner.deleted.store(true, Ordering::Relaxed);
    }

    pub fn compacting(&self) -> bool {
        self.inner.compacting.load(Ordering::Relaxed)
    }

    pub fn set_compacting(&self, compacting: bool) {
        self.inner.compacting.store(compacting, Ordering::Relaxed);
    }

    /// Returns a reference to the [FileMeta].
    pub fn meta_ref(&self) -> &FileMeta {
        &self.inner.meta
    }

    pub fn size(&self) -> u64 {
        self.inner.meta.file_size
    }

    pub fn index_size(&self) -> u64 {
        self.inner.meta.index_file_size
    }

    pub fn num_rows(&self) -> usize {
        self.inner.meta.num_rows as usize
    }
}

/// Inner data of [FileHandle].
///
/// Contains meta of the file, and other mutable info like whether the file is compacting.
struct FileHandleInner {
    meta: FileMeta,
    compacting: AtomicBool,
    deleted: AtomicBool,
    file_purger: FilePurgerRef,
}

impl Drop for FileHandleInner {
    fn drop(&mut self) {
        if self.deleted.load(Ordering::Relaxed) {
            self.file_purger.send_request(PurgeRequest {
                file_meta: self.meta.clone(),
            });
        }
    }
}

impl FileHandleInner {
    fn new(meta: FileMeta, file_purger: FilePurgerRef) -> FileHandleInner {
        FileHandleInner {
            meta,
            compacting: AtomicBool::new(false),
            deleted: AtomicBool::new(false),
            file_purger,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_id() {
        let id = FileId::random();
        let uuid_str = id.to_string();
        assert_eq!(id.0.to_string(), uuid_str);

        let parsed = FileId::parse_str(&uuid_str).unwrap();
        assert_eq!(id, parsed);
        let parsed = uuid_str.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_file_id_serialization() {
        let id = FileId::random();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(format!("\"{id}\""), json);

        let parsed = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_file_id_as_parquet() {
        let id = FileId::from_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
        assert_eq!(
            "67e55044-10b1-426f-9247-bb680e5fe0c8.parquet",
            id.as_parquet()
        );
    }

    fn create_file_meta(file_id: FileId, level: Level) -> FileMeta {
        FileMeta {
            region_id: 0.into(),
            file_id,
            time_range: FileTimeRange::default(),
            level,
            file_size: 0,
            available_indexes: SmallVec::from_iter([IndexType::InvertedIndex]),
            index_file_size: 0,
            num_rows: 0,
            num_row_groups: 0,
            sequence: None,
        }
    }

    #[test]
    fn test_deserialize_file_meta() {
        let file_meta = create_file_meta(FileId::random(), 0);
        let serialized_file_meta = serde_json::to_string(&file_meta).unwrap();
        let deserialized_file_meta = serde_json::from_str(&serialized_file_meta);
        assert_eq!(file_meta, deserialized_file_meta.unwrap());
    }

    #[test]
    fn test_deserialize_from_string() {
        let json_file_meta = "{\"region_id\":0,\"file_id\":\"bc5896ec-e4d8-4017-a80d-f2de73188d55\",\
        \"time_range\":[{\"value\":0,\"unit\":\"Millisecond\"},{\"value\":0,\"unit\":\"Millisecond\"}],\
        \"available_indexes\":[\"InvertedIndex\"],\"level\":0}";
        let file_meta = create_file_meta(
            FileId::from_str("bc5896ec-e4d8-4017-a80d-f2de73188d55").unwrap(),
            0,
        );
        let deserialized_file_meta: FileMeta = serde_json::from_str(json_file_meta).unwrap();
        assert_eq!(file_meta, deserialized_file_meta);
    }
}
