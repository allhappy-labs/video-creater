//! Unsupported platforms fail closed instead of path-based publication/deletion.
use super::*;
use std::fs::Metadata;

pub struct PreparedExportOutput;
pub struct ExportPublicationOwner;

fn unsupported() -> ExportDestinationError {
    ExportDestinationError::Io(
        "safe descriptor-relative export publication is unavailable on this platform".into(),
    )
}

pub fn prepare_export_output(
    request: ExportMaterialization<'_>,
) -> Result<PreparedExportOutput, ExportDestinationError> {
    prepare_export_output_cancellable(request, &mut || true)
}
pub(crate) fn prepare_export_output_cancellable(
    _request: ExportMaterialization<'_>,
    keep_working: &mut dyn FnMut() -> bool,
) -> Result<PreparedExportOutput, ExportDestinationError> {
    if keep_working() {
        Err(unsupported())
    } else {
        Err(ExportDestinationError::Cancelled)
    }
}
impl PreparedExportOutput {
    pub fn publish(self) -> Result<MaterializedExport, ExportDestinationError> {
        Err(unsupported())
    }
    pub fn publication_owner(&self) -> Result<ExportPublicationOwner, ExportDestinationError> {
        Err(unsupported())
    }
    pub(super) fn publish_with_hook(
        self,
        _hook: &mut dyn FnMut(&Path),
    ) -> Result<MaterializedExport, ExportDestinationError> {
        Err(unsupported())
    }
}
impl ExportPublicationOwner {
    pub fn rollback(&self, _path: &Path) -> Result<(), ExportDestinationError> {
        Err(unsupported())
    }
}
pub fn remove_export_output_if_owned(_path: &Path, _owner: &Metadata) {}
