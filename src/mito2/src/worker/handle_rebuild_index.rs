use std::collections::HashMap;
use std::sync::Arc;

use common_telemetry::debug;
use puffin::puffin_manager;

use crate::access_layer::{self, OperationType};
use crate::cache::write_cache;
use crate::error::RegionNotFoundSnafu;
use crate::region::{MitoRegion, MitoRegionRef};
use crate::request::{self, RegionBuildIndexRequest};
use crate::sst::file::{FileHandle, FileId};
use crate::sst::index::{IndexBuildTask, IndexBuildType, IndexerBuilderImpl};
use crate::sst::parquet::WriteOptions;
use crate::worker::RegionWorkerLoop;

impl<S> RegionWorkerLoop<S> {
    pub(crate) fn new_index_build_task(
        &self,
        region: &MitoRegionRef,
        file: FileHandle,
        build_type: IndexBuildType,
    ) -> IndexBuildTask {
        let version = region.version();
        let access_layer = region.access_layer.clone();

        let puffin_manager = if let Some(write_cache) = self.cache_manager.write_cache() {
            write_cache.build_puffin_manager(region.region_id)
        } else {
            access_layer.build_puffin_manager()
        };

        let indexer_builder_ref = Arc::new(IndexerBuilderImpl {
            build_type,
            metadata: version.metadata.clone(),
            inverted_index_config: self.config.inverted_index.clone(),
            fulltext_index_config: self.config.fulltext_index.clone(),
            bloom_filter_index_config: self.config.bloom_filter_index.clone(),
            index_options: version.options.index_options.clone(),
            row_group_size: WriteOptions::default().row_group_size,
            intermediate_manager: self.intermediate_manager.clone(),
            puffin_manager,
        });

        IndexBuildTask {
            file_meta: file.meta_ref().clone(),
            flushed_entry_id: Some(version.flushed_entry_id),
            flushed_sequence: Some(version.flushed_sequence),
            access_layer: access_layer.clone(),
            file_purger: file.file_purger(),
            request_sender: self.sender.clone(),
            indexer_builder: indexer_builder_ref.clone(),
        }
    }

    pub(crate) async fn handle_rebuild_index(&mut self, request: RegionBuildIndexRequest) {
        debug!(
            "rebuild index for file id = {:?}, type = {:?}",
            request.region_id, request.build_type
        );
        let region_id = request.region_id;
        let Some(region) = self.regions.get_region(region_id) else {
            return;
        };

        let version = region.version();

        let all_files: HashMap<FileId, FileHandle> = version
            .ssts
            .levels()
            .iter()
            .flat_map(|level| level.files.iter())
            .filter(|(_, handle)| !handle.is_deleted())
            .map(|(id, handle)| (*id, handle.clone()))
            .collect();

        let build_tasks = if request.file_metas.is_empty() {
            all_files.values().cloned().collect::<Vec<_>>()
        } else {
            request
                .file_metas
                .iter()
                .filter_map(|meta| all_files.get(&meta.file_id).cloned())
                .collect::<Vec<_>>()
        };

        for file_handle in build_tasks {
            let _ = self
                .index_build_scheduler
                .schedule_build(self.new_index_build_task(
                    &region,
                    file_handle,
                    request.build_type.clone(),
                ));
        }
    }
}
