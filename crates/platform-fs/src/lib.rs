use std::{
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_TEMP_ATTEMPTS: usize = 128;
const TEMP_MARKER: &str = ".diagramdesigner-next-save-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitMode {
    Created,
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityLevel {
    FileAndDirectorySynced,
    FileSyncedAndPlatformCommitFlushed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSaveReport {
    pub mode: CommitMode,
    pub durability: DurabilityLevel,
    pub cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStage {
    InspectDestination,
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    CommitNew,
    ReplaceExisting,
    SyncParentDirectory,
}

#[derive(Debug)]
pub struct AtomicSaveError {
    pub stage: SaveStage,
    pub path: PathBuf,
    pub committed: bool,
    source: io::Error,
}

impl AtomicSaveError {
    fn new(stage: SaveStage, path: impl Into<PathBuf>, committed: bool, source: io::Error) -> Self {
        Self {
            stage,
            path: path.into(),
            committed,
            source,
        }
    }

    pub fn io_error(&self) -> &io::Error {
        &self.source
    }
}

impl fmt::Display for AtomicSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let commit_state = if self.committed {
            "the destination may already contain the new data"
        } else {
            "the destination was not committed"
        };
        write!(
            f,
            "atomic save failed during {:?} for {} ({commit_state}): {}",
            self.stage,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for AtomicSaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupReport {
    pub removed: usize,
    pub failures: Vec<CleanupFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupFailure {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestinationState {
    Missing,
    ExistingRegularFile,
}

#[derive(Debug, Default, Clone, Copy)]
struct SaveHooks {
    fail_after_temp_sync: bool,
}

/// Persist complete, already-verified document bytes through the ADR-015 boundary.
///
/// The temporary file is always a unique sibling of `destination`, so the final
/// commit never crosses filesystems. Existing files are never deleted before the
/// native replace operation. A returned error with `committed == false` guarantees
/// the commit point was not crossed. If `committed == true`, the destination was
/// already committed and only the post-commit durability step failed.
pub fn atomic_save(
    destination: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<AtomicSaveReport, AtomicSaveError> {
    atomic_save_inner(destination.as_ref(), bytes, SaveHooks::default())
}

/// Conservatively remove old sibling temporaries belonging to one destination.
///
/// Only regular files matching this adapter's exact temp-file prefix are eligible.
/// Non-Unicode destination names are skipped instead of using a lossy comparison.
pub fn cleanup_stale_siblings(
    destination: impl AsRef<Path>,
    older_than: Duration,
) -> Result<CleanupReport, AtomicSaveError> {
    let destination = destination.as_ref();
    let parent = normalized_parent(destination)?;
    let Some(prefix) = cleanup_prefix(destination) else {
        return Ok(CleanupReport::default());
    };

    let entries = fs::read_dir(parent).map_err(|source| {
        AtomicSaveError::new(SaveStage::InspectDestination, parent, false, source)
    })?;
    let now = SystemTime::now();
    let mut report = CleanupReport::default();

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(source) => {
                report.failures.push(CleanupFailure {
                    path: parent.to_path_buf(),
                    message: source.to_string(),
                });
                continue;
            }
        };

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".tmp") {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                report.failures.push(CleanupFailure {
                    path: entry.path(),
                    message: source.to_string(),
                });
                continue;
            }
        };
        if !metadata.is_file() {
            continue;
        }

        let old_enough = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= older_than);
        if !old_enough {
            continue;
        }

        match fs::remove_file(entry.path()) {
            Ok(()) => report.removed += 1,
            Err(source) => report.failures.push(CleanupFailure {
                path: entry.path(),
                message: source.to_string(),
            }),
        }
    }

    Ok(report)
}

fn atomic_save_inner(
    destination: &Path,
    bytes: &[u8],
    hooks: SaveHooks,
) -> Result<AtomicSaveReport, AtomicSaveError> {
    let parent = normalized_parent(destination)?;
    let state = inspect_destination(destination)?;
    let (mut temporary, temporary_path) = create_unique_sibling(destination, parent)?;

    if let Err(source) = temporary.write_all(bytes) {
        let _ = fs::remove_file(&temporary_path);
        return Err(AtomicSaveError::new(
            SaveStage::WriteTemporary,
            &temporary_path,
            false,
            source,
        ));
    }
    if let Err(source) = temporary.flush() {
        let _ = fs::remove_file(&temporary_path);
        return Err(AtomicSaveError::new(
            SaveStage::WriteTemporary,
            &temporary_path,
            false,
            source,
        ));
    }
    if let Err(source) = temporary.sync_all() {
        let _ = fs::remove_file(&temporary_path);
        return Err(AtomicSaveError::new(
            SaveStage::SyncTemporary,
            &temporary_path,
            false,
            source,
        ));
    }
    drop(temporary);

    if hooks.fail_after_temp_sync {
        let _ = fs::remove_file(&temporary_path);
        return Err(AtomicSaveError::new(
            match state {
                DestinationState::Missing => SaveStage::CommitNew,
                DestinationState::ExistingRegularFile => SaveStage::ReplaceExisting,
            },
            destination,
            false,
            io::Error::other("injected pre-commit failure"),
        ));
    }

    let (mode, cleanup_warning) = match state {
        DestinationState::Missing => {
            if let Err(source) = commit_new(&temporary_path, destination) {
                let _ = fs::remove_file(&temporary_path);
                return Err(AtomicSaveError::new(
                    SaveStage::CommitNew,
                    destination,
                    false,
                    source,
                ));
            }
            let cleanup_warning = cleanup_committed_temp_if_needed(&temporary_path);
            (CommitMode::Created, cleanup_warning)
        }
        DestinationState::ExistingRegularFile => {
            if let Err(source) = replace_existing(&temporary_path, destination) {
                let _ = fs::remove_file(&temporary_path);
                return Err(AtomicSaveError::new(
                    SaveStage::ReplaceExisting,
                    destination,
                    false,
                    source,
                ));
            }
            (CommitMode::Replaced, None)
        }
    };

    let durability = sync_parent_after_commit(parent, destination)?;
    Ok(AtomicSaveReport {
        mode,
        durability,
        cleanup_warning,
    })
}

fn inspect_destination(destination: &Path) -> Result<DestinationState, AtomicSaveError> {
    if destination.file_name().is_none() {
        return Err(AtomicSaveError::new(
            SaveStage::InspectDestination,
            destination,
            false,
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name"),
        ));
    }

    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(DestinationState::ExistingRegularFile),
        Ok(_) => Err(AtomicSaveError::new(
            SaveStage::InspectDestination,
            destination,
            false,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination exists but is not a regular file",
            ),
        )),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(DestinationState::Missing),
        Err(source) => Err(AtomicSaveError::new(
            SaveStage::InspectDestination,
            destination,
            false,
            source,
        )),
    }
}

fn normalized_parent(destination: &Path) -> Result<&Path, AtomicSaveError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };

    if !parent.is_dir() {
        return Err(AtomicSaveError::new(
            SaveStage::InspectDestination,
            parent,
            false,
            io::Error::new(
                io::ErrorKind::NotFound,
                "destination parent directory does not exist",
            ),
        ));
    }
    Ok(parent)
}

fn create_unique_sibling(
    destination: &Path,
    parent: &Path,
) -> Result<(File, PathBuf), AtomicSaveError> {
    let file_name = destination.file_name().expect("destination validated");

    for _ in 0..MAX_TEMP_ATTEMPTS {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = OsString::from(".");
        name.push(file_name);
        name.push(format!(
            "{TEMP_MARKER}{}-{sequence}.tmp",
            std::process::id()
        ));
        let path = parent.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(AtomicSaveError::new(
                    SaveStage::CreateTemporary,
                    path,
                    false,
                    source,
                ));
            }
        }
    }

    Err(AtomicSaveError::new(
        SaveStage::CreateTemporary,
        parent,
        false,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique sibling temporary file",
        ),
    ))
}

#[cfg(unix)]
fn commit_new(temporary: &Path, destination: &Path) -> io::Result<()> {
    // Linking creates the destination name atomically and fails if another writer
    // won the race. The temporary is a sibling, so the hard link remains on the
    // same filesystem. ext4 and APFS both support this primitive.
    fs::hard_link(temporary, destination)
}

#[cfg(windows)]
fn commit_new(temporary: &Path, destination: &Path) -> io::Result<()> {
    move_file_ex(temporary, destination, MOVEFILE_WRITE_THROUGH)
}

#[cfg(unix)]
fn replace_existing(temporary: &Path, destination: &Path) -> io::Result<()> {
    // POSIX same-filesystem rename replaces the destination atomically. No
    // delete-before-rename path is used.
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn replace_existing(temporary: &Path, destination: &Path) -> io::Result<()> {
    move_file_ex(
        temporary,
        destination,
        MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
    )
}

#[cfg(unix)]
fn cleanup_committed_temp_if_needed(temporary: &Path) -> Option<String> {
    // commit_new uses hard_link on Unix, so the temp name remains after the
    // destination name has been committed. Failure to unlink it must not roll
    // back or damage the already-valid destination.
    fs::remove_file(temporary)
        .err()
        .map(|error| error.to_string())
}

#[cfg(windows)]
fn cleanup_committed_temp_if_needed(_temporary: &Path) -> Option<String> {
    // MoveFileEx consumes the source name on success.
    None
}

#[cfg(unix)]
fn sync_parent_after_commit(
    parent: &Path,
    destination: &Path,
) -> Result<DurabilityLevel, AtomicSaveError> {
    let directory = File::open(parent).map_err(|source| {
        AtomicSaveError::new(SaveStage::SyncParentDirectory, destination, true, source)
    })?;
    directory.sync_all().map_err(|source| {
        AtomicSaveError::new(SaveStage::SyncParentDirectory, destination, true, source)
    })?;
    Ok(DurabilityLevel::FileAndDirectorySynced)
}

#[cfg(windows)]
fn sync_parent_after_commit(
    _parent: &Path,
    _destination: &Path,
) -> Result<DurabilityLevel, AtomicSaveError> {
    // The temp file is sync_all()'d before commit and MoveFileExW is requested
    // with MOVEFILE_WRITE_THROUGH. Windows does not expose the POSIX directory
    // fsync model through std::fs, so we report the exact guarantee used here.
    Ok(DurabilityLevel::FileSyncedAndPlatformCommitFlushed)
}

fn cleanup_prefix(destination: &Path) -> Option<String> {
    let name = destination.file_name()?.to_str()?;
    Some(format!(".{name}{TEMP_MARKER}"))
}

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
#[cfg(windows)]
const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

#[cfg(windows)]
fn move_file_ex(source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            lp_existing_file_name: *const u16,
            lp_new_file_name: *const u16,
            dw_flags: u32,
        ) -> i32;
    }

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both buffers are NUL-terminated, live for the duration of the call,
    // and are only read by MoveFileExW.
    let result = unsafe { MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), flags) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "diagramdesigner-next-platform-fs-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn creates_new_file_without_overwriting_race_path() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");

        let report = atomic_save(&destination, b"first package").unwrap();

        assert_eq!(report.mode, CommitMode::Created);
        assert_eq!(fs::read(&destination).unwrap(), b"first package");
        assert!(report.cleanup_warning.is_none());
    }

    #[test]
    fn replaces_existing_file_without_delete_before_replace() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");
        fs::write(&destination, b"old package").unwrap();

        let report = atomic_save(&destination, b"new package").unwrap();

        assert_eq!(report.mode, CommitMode::Replaced);
        assert_eq!(fs::read(&destination).unwrap(), b"new package");
    }

    #[test]
    fn injected_precommit_failure_preserves_existing_destination() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");
        fs::write(&destination, b"last valid package").unwrap();

        let error = atomic_save_inner(
            &destination,
            b"replacement that must not commit",
            SaveHooks {
                fail_after_temp_sync: true,
            },
        )
        .unwrap_err();

        assert_eq!(error.stage, SaveStage::ReplaceExisting);
        assert!(!error.committed);
        assert_eq!(fs::read(&destination).unwrap(), b"last valid package");
    }

    #[test]
    fn injected_new_file_commit_failure_leaves_destination_absent() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");

        let error = atomic_save_inner(
            &destination,
            b"package",
            SaveHooks {
                fail_after_temp_sync: true,
            },
        )
        .unwrap_err();

        assert_eq!(error.stage, SaveStage::CommitNew);
        assert!(!error.committed);
        assert!(!destination.exists());
    }

    #[test]
    fn rejects_non_regular_destination_before_temp_commit() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");
        fs::create_dir(&destination).unwrap();

        let error = atomic_save(&destination, b"package").unwrap_err();

        assert_eq!(error.stage, SaveStage::InspectDestination);
        assert!(!error.committed);
        assert!(destination.is_dir());
    }

    #[test]
    fn stale_cleanup_only_removes_our_matching_regular_siblings() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");
        let prefix = cleanup_prefix(&destination).unwrap();
        let stale = directory.path.join(format!("{prefix}old.tmp"));
        let unrelated = directory.path.join("unrelated.tmp");
        let similarly_named = directory.path.join(format!("{prefix}not-a-temp"));
        fs::write(&stale, b"stale").unwrap();
        fs::write(&unrelated, b"keep").unwrap();
        fs::write(&similarly_named, b"keep").unwrap();

        let report = cleanup_stale_siblings(&destination, Duration::ZERO).unwrap();

        assert_eq!(report.removed, 1);
        assert!(report.failures.is_empty());
        assert!(!stale.exists());
        assert!(unrelated.exists());
        assert!(similarly_named.exists());
    }

    #[cfg(unix)]
    #[test]
    fn unix_reports_file_and_directory_durability() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");

        let report = atomic_save(&destination, b"package").unwrap();

        assert_eq!(report.durability, DurabilityLevel::FileAndDirectorySynced);
    }

    #[cfg(windows)]
    #[test]
    fn windows_replace_uses_native_same_volume_replace_path() {
        let directory = TestDirectory::new();
        let destination = directory.path.join("drawing.ddnx");
        fs::write(&destination, b"old").unwrap();

        let report = atomic_save(&destination, b"new").unwrap();

        assert_eq!(report.mode, CommitMode::Replaced);
        assert_eq!(
            report.durability,
            DurabilityLevel::FileSyncedAndPlatformCommitFlushed
        );
        assert_eq!(fs::read(&destination).unwrap(), b"new");
    }
}
