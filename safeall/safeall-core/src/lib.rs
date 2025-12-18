pub const MAINTAINER_EMAIL: &str = "christoph.ungricht@outlook.com";

#[inline]
fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[derive(Debug)]
pub enum ReadDirType {
    DirectoriesOnly,
    FilesOnly,
}

#[derive(Debug)]
pub struct RecursiveReadDir {
    for_root: std::path::PathBuf,
    readdir_type: ReadDirType,
    next_readdirs: std::collections::VecDeque<std::path::PathBuf>,
    current_readdir: std::fs::ReadDir,
    current_dirpath: std::path::PathBuf,
}

impl RecursiveReadDir {
    pub fn root_directory(&self) -> &std::path::Path {
        self.for_root.as_ref()
    }

    pub fn try_new<P: AsRef<std::path::Path>>(
        directory: P,
        readdir_type: ReadDirType,
    ) -> Result<Self, std::io::Error> {
        let directory: &std::path::Path = directory.as_ref();
        let current_readdir = std::fs::read_dir(directory)?;
        Ok(Self {
            for_root: directory.to_owned(),
            readdir_type,
            current_readdir,
            next_readdirs: std::collections::VecDeque::new(),
            current_dirpath: directory.to_owned(),
        })
    }
}

impl Iterator for RecursiveReadDir {
    type Item = Result<std::path::PathBuf, (std::path::PathBuf, FileBackupError)>;

    fn next(&mut self) -> Option<Self::Item> {
        use FileBackupError as E;
        'drain_current_readdir: loop {
            for entry in &mut self.current_readdir {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir() {
                            self.next_readdirs.push_back(path);
                        } else if matches!(self.readdir_type, ReadDirType::FilesOnly) {
                            return Some(Ok(path));
                        }
                    }
                    Err(error) => {
                        return Some(Err((
                            self.current_dirpath.clone(),
                            E::CannotGetDirEntry(error),
                        )));
                    }
                }
            }
            debug_assert!(
                self.current_readdir.next().is_none(),
                "The `current_readdir` must be empty so we can create a new one"
            );
            if let Some(next_readdir) = self.next_readdirs.pop_front() {
                debug_assert!(next_readdir.is_dir(), "Must be a directory.");
                match std::fs::read_dir(&next_readdir) {
                    Ok(readdir) => {
                        self.current_readdir = readdir;
                        self.current_dirpath = next_readdir.clone();
                        match self.readdir_type {
                            ReadDirType::FilesOnly => continue 'drain_current_readdir,
                            ReadDirType::DirectoriesOnly => {
                                return Some(Ok(next_readdir));
                            }
                        }
                    }
                    Err(error) => {
                        return Some(Err((next_readdir, E::CannotReadDirectoryContent(error))));
                    }
                }
            }
            debug_assert!(
                self.current_readdir.next().is_none(),
                "Current readdir must be empty"
            );
            debug_assert!(
                self.next_readdirs.is_empty(),
                "Next readdirs must be empty."
            );
            return None;
        }
    }
}

#[derive(Debug)]
pub enum InvariantError {
    CannotStripPrefixOfPath(
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::StripPrefixError,
    ),
}

impl std::error::Error for InvariantError {}

impl std::fmt::Display for InvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvariantError::CannotStripPrefixOfPath(path_root, path, strip_prefix_error) => {
                write!(
                    f,
                    "cannot strip prefix \"{}\" from \"{}\" => {strip_prefix_error}",
                    path_root.display(),
                    path.display()
                )
            }
        }
    }
}

#[derive(Debug)]
pub enum FileBackupError {
    CannotCreateDestinationDir(std::path::PathBuf, std::io::Error),
    CannotReadDirectoryContent(std::io::Error),
    CannotGetDirEntry(std::io::Error),
    DestinationForSourceDirExistsAsFile(std::path::PathBuf),
    CannotCopyFile(std::path::PathBuf, std::io::Error),
    CouldNotGenerateDestinationPath(std::path::StripPrefixError),
    InvariantBroken(InvariantError),
    CannotDeleteDirectory(std::io::Error),
    CannotDeleteFile(std::io::Error),
}

impl std::error::Error for FileBackupError {}

impl std::fmt::Display for FileBackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileBackupError::CannotCreateDestinationDir(path, error) => {
                write!(
                    f,
                    "cannot create destination path \"{}\" because {error}",
                    path.display()
                )
            }
            FileBackupError::CannotReadDirectoryContent(error) => {
                write!(f, "could not read source directory because {error}")
            }
            FileBackupError::CannotGetDirEntry(error) => {
                write!(f, "could not read entries of directory because {error}")
            }
            FileBackupError::DestinationForSourceDirExistsAsFile(destination_dir) => write!(
                f,
                "could not create a new destination directory \"{}\" as the destination already exists but not as a directory",
                destination_dir.display()
            ),
            FileBackupError::CannotCopyFile(destination_file, error) => write!(
                f,
                "could not copy file to destination \"{}\" because {error}",
                destination_file.display()
            ),
            FileBackupError::CouldNotGenerateDestinationPath(strip_prefix_error) => write!(
                f,
                "could not generate destination path because {strip_prefix_error}"
            ),
            FileBackupError::InvariantBroken(error) => write!(
                f,
                "there is some problem with the program. Contact the maintainer {MAINTAINER_EMAIL} to fix the error. Error: {error}"
            ),
            FileBackupError::CannotDeleteDirectory(error) => {
                write!(f, "cannot delet directory because {error}")
            }
            FileBackupError::CannotDeleteFile(error) => {
                write!(f, "cannot delete file because {error}")
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    FileBackupErrors(Vec<(std::path::PathBuf, FileBackupError)>),
    SourceRootPathDoesNotExist(std::path::PathBuf),
    CannotReadDirectoryContent(std::path::PathBuf, std::io::Error),
    CannotCreateRootDestinationDir(std::path::PathBuf, std::io::Error),
    RootDestinatinIsNotADirectory(std::path::PathBuf),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::FileBackupErrors(errors) => {
                errors.iter().enumerate().try_for_each(|(i, (p, e))| {
                    let end = if i == errors.len() - 1 { "" } else { "\n" };
                    write!(f, "could not backup \"{}\" because {e}{end}", p.display())
                })
            }
            Error::SourceRootPathDoesNotExist(path_buf) => write!(
                f,
                "the specified source directory \"{}\" does not exists",
                path_buf.display()
            ),
            Error::CannotCreateRootDestinationDir(path_buf, error) => write!(
                f,
                "cannot create a new destination directory \"{}\" because {error}",
                path_buf.display()
            ),
            Error::RootDestinatinIsNotADirectory(path_buf) => write!(
                f,
                "specified destination \"{}\" is not a directory but a file",
                path_buf.display()
            ),
            Error::CannotReadDirectoryContent(path_buf, error) => write!(
                f,
                "cannot iterate through directory\"{}\" because {error}",
                path_buf.display()
            ),
        }
    }
}

async fn backup_all_files(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    not_existing_destination_directories: &[&std::path::Path],
    message_sender: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<Vec<(std::path::PathBuf, FileBackupError)>, Error> {
    debug_assert!(source_directory_root.is_dir(), "Source is not a dir");
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );
    let source_recurse_files =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::FilesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e))?;

    use futures::stream::StreamExt;
    let num_files = futures::stream::iter(source_recurse_files).count().await;
    let source_recurse_files =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::FilesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e))?;
    let done = std::sync::atomic::AtomicUsize::new(0);
    let done = &done;
    let errors: Vec<_> = futures::stream::iter(source_recurse_files)
        .map(|source_file| async move {
            let source_file = source_file?;
            if not_existing_destination_directories
                .iter()
                .any(|d| source_file.starts_with(d))
            {
                // Do not try to copy files for directories that do not exist
                return Ok(());
            }
            backup_single_file(
                source_directory_root,
                destination_directory_root,
                source_file,
                message_sender,
            )
            .await
            .inspect(|_| {
                message_sender
                    .send(Message::Progress {
                        progress: Progress::Copy,
                        done: done.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                        total: num_files,
                    })
                    .ok();
            })
        })
        .buffer_unordered(cpu_count())
        .filter_map(async move |res| res.err())
        .collect()
        .await;
    Ok(errors)
}

async fn backup_single_file(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    source_file: std::path::PathBuf,
    message_sender: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<(), (std::path::PathBuf, FileBackupError)> {
    debug_assert!(!source_file.is_dir(), "Must be a file or a symbolic link");

    let new_destination_file = get_destination_file_path(
        destination_directory_root,
        source_directory_root,
        &source_file,
    )?;

    copy_if_newer(&source_file, &new_destination_file, message_sender).await
}

#[must_use]
async fn create_all_directories_in_destination(
    source_recurse_directories: RecursiveReadDir,
    destination_directory_root: &std::path::Path,
) -> Vec<(std::path::PathBuf, FileBackupError)> {
    debug_assert!(
        source_recurse_directories.root_directory().is_dir(),
        "Source is not a dir"
    );
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );

    let mut errors = vec![];
    let source_directory_root = source_recurse_directories.root_directory().to_owned();
    for source_directory in source_recurse_directories {
        match source_directory {
            Ok(source_directory) => {
                if errors.iter().any(|(d, _)| source_directory.starts_with(d)) {
                    // Do do not recurse
                    continue;
                }

                if let Err(err) = create_single_directory(
                    &source_directory_root,
                    destination_directory_root,
                    source_directory,
                )
                .await
                {
                    errors.push(err);
                }
            }
            Err(err) => errors.push(err),
        }
    }
    errors
}

async fn create_single_directory(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    source_directory: std::path::PathBuf,
) -> Result<(), (std::path::PathBuf, FileBackupError)> {
    debug_assert!(source_directory.is_dir(), "Must be a directory");
    let new_destination_file = get_destination_file_path(
        destination_directory_root,
        source_directory_root,
        &source_directory,
    )?;

    if new_destination_file.exists() {
        // If it already exists it must be a directory too
        if new_destination_file.is_dir() {
            Ok(())
        } else {
            Err((
                source_directory,
                FileBackupError::DestinationForSourceDirExistsAsFile(new_destination_file),
            ))
        }
    } else {
        tokio::fs::create_dir(&new_destination_file)
            .await
            .map_err(|e| {
                (
                    source_directory,
                    FileBackupError::CannotCreateDestinationDir(new_destination_file, e),
                )
            })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileMetaData {
    modified: Option<std::time::SystemTime>,
    length: u64,
    file_type: std::fs::FileType,
    permissions: std::fs::Permissions,
}

impl FileMetaData {
    async fn try_new(path: &std::path::Path) -> Option<Self> {
        let metadata = tokio::fs::metadata(path).await.ok()?;
        let modified = metadata.modified().ok();
        Some(Self {
            modified,
            length: metadata.len(),
            file_type: metadata.file_type(),
            permissions: metadata.permissions(),
        })
    }
}

async fn get_hash(path: &std::path::Path) -> Option<blake3::Hash> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut hasher = blake3::Hasher::new();
        let mut file = std::fs::File::open(path).ok()?;
        std::io::copy(&mut file, &mut hasher).ok()?;
        let res = hasher.finalize();
        Some(res)
    })
    .await
    .ok()?
}

pub enum Progress {
    Copy,
    Delete,
}

pub enum Message {
    // TODO: Maybe we should send the errors here?
    Warning(String),
    Info(String),
    Progress {
        progress: Progress,
        done: usize,
        total: usize,
    },
}

async fn skip_copy(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
    source_metadata: &Option<FileMetaData>,
    message_sender: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> bool {
    if !destination_file.exists() {
        return false;
    }
    let destination_metadata = FileMetaData::try_new(destination_file).await;
    if destination_metadata.is_none() && source_metadata.is_some()
        || destination_metadata.is_some() && source_metadata.is_none()
    {
        message_sender.send(Message::Warning(format!(
            "we can get the metadata from one file but not the other. We just try to copy it anyway. Files: \"{}\" and \"{}\"",
            source_file.display(),
            destination_file.display()
            ))).ok();
        return false;
    }

    if *source_metadata != FileMetaData::try_new(destination_file).await {
        return false;
    }

    if let (Some(src_hash), Some(dest_hash)) = (
        get_hash(source_file).await,
        get_hash(destination_file).await,
    ) {
        if src_hash != dest_hash {
            return false;
        }
    } else {
        eprintln!(
            "WARNING: Cannot get hash of either {} or {}. We just try to copy the file anyway.",
            source_file.display(),
            destination_file.display()
        );
    }

    true
}

async fn copy_if_newer(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
    message_sender: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<(), (std::path::PathBuf, FileBackupError)> {
    debug_assert!(
        !source_file.is_dir(),
        "Source file must not be a directory."
    );
    debug_assert!(
        !destination_file.is_dir(),
        "Destinaion file must not be a directory."
    );

    let source_metadata = FileMetaData::try_new(source_file).await;
    if skip_copy(
        source_file,
        destination_file,
        &source_metadata,
        message_sender,
    )
    .await
    {
        message_sender
            .send(Message::Info(format!(
                "Not copying \"{}\" as destination \"{}\" did not change.",
                source_file.display(),
                destination_file.display()
            )))
            .ok();
        return Ok(());
    }

    tokio::fs::copy(source_file, destination_file)
        .await
        .map_err(|e| {
            (
                source_file.to_owned(),
                FileBackupError::CannotCopyFile(destination_file.to_owned(), e),
            )
        })?;
    message_sender
        .send(Message::Info(format!(
            "Copied \"{}\" to \"{}\"",
            source_file.display(),
            destination_file.display()
        )))
        .ok();
    if set_modified_time(&source_metadata, destination_file)
        .await
        .is_none()
    {
        message_sender
            .send(Message::Warning(format!(
                "Could not copy modified time from \"{}\" to destination file \"{}\"",
                source_file.display(),
                destination_file.display()
            )))
            .ok();
    }

    Ok(())
}

async fn set_modified_time(
    source_metadata: &Option<FileMetaData>,
    destination_file: &std::path::Path,
) -> Option<()> {
    if let Some(source_metadata) = source_metadata
        && let Some(modified) = source_metadata.modified
    {
        let destination_file = destination_file.to_owned();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(destination_file).ok()?;
            file.set_modified(modified).ok()
        })
        .await
        .ok()?
    } else {
        // There is no metadata or modified date of source, so we do not copy it
        Some(())
    }
}

async fn get_paths_in_destinatination_but_not_in_source(
    recurse_source: RecursiveReadDir,
    recurse_destination: RecursiveReadDir,
) -> Result<Vec<std::path::PathBuf>, (std::path::PathBuf, FileBackupError)> {
    let source_root_path = recurse_source.root_directory().to_owned();
    let source_root_path = &source_root_path;
    let destination_root_path = recurse_destination.root_directory().to_owned();
    let destination_root_path = &destination_root_path;
    use futures::stream::TryStreamExt;
    let source_files: std::collections::HashSet<_> = futures::stream::iter(recurse_source)
        .and_then(async |path| {
            path.strip_prefix(source_root_path)
                .map(|p| p.to_owned())
                .map_err(|e| {
                    (
                        path.clone(),
                        FileBackupError::InvariantBroken(InvariantError::CannotStripPrefixOfPath(
                            source_root_path.to_owned(),
                            path.clone(),
                            e,
                        )),
                    )
                })
        })
        .try_collect()
        .await?;
    let destination_files: std::collections::HashSet<_> =
        futures::stream::iter(recurse_destination)
            .and_then(async |path| {
                path.strip_prefix(destination_root_path)
                    .map(|p| p.to_owned())
                    .map_err(|e| {
                        (
                            path.to_owned(),
                            FileBackupError::InvariantBroken(
                                InvariantError::CannotStripPrefixOfPath(
                                    destination_root_path.clone(),
                                    path.clone(),
                                    e,
                                ),
                            ),
                        )
                    })
            })
            .try_collect()
            .await?;
    let mut res: Vec<_> = (&destination_files - &source_files)
        .iter()
        .map(|p| [destination_root_path, p].iter().collect())
        .collect();
    res.sort(); // Such that foo/bar/baz is after foo/bar
    Ok(res)
}
fn validate_root_paths<P: AsRef<std::path::Path>>(
    source_directory_root: P,
    destination_directory_root: P,
) -> Result<(), Error> {
    let source_directory_root = source_directory_root.as_ref();
    let destination_directory_root = destination_directory_root.as_ref();

    if !source_directory_root.exists() {
        return Err(Error::SourceRootPathDoesNotExist(
            source_directory_root.to_owned(),
        ));
    }
    if !destination_directory_root.exists()
        && let Err(e) = std::fs::create_dir_all(destination_directory_root)
    {
        return Err(Error::CannotCreateRootDestinationDir(
            destination_directory_root.to_owned(),
            e,
        ));
    }
    if !destination_directory_root.is_dir() {
        return Err(Error::RootDestinatinIsNotADirectory(
            destination_directory_root.to_owned(),
        ));
    }
    Ok(())
}
async fn backup<P: AsRef<std::path::Path>>(
    source_directory_root: P,
    destination_directory_root: P,
    message_sender: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<(), Error> {
    let source_directory_root = source_directory_root.as_ref();
    let destination_directory_root = destination_directory_root.as_ref();

    let source_recurse_directories =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::DirectoriesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e))?;

    let mut errors = create_all_directories_in_destination(
        source_recurse_directories,
        destination_directory_root,
    )
    .await;
    let non_exsting_destination_directories: Vec<_> =
        errors.iter().map(|(p, _)| p.as_path()).collect();

    let mut file_backup_errors = backup_all_files(
        source_directory_root,
        destination_directory_root,
        non_exsting_destination_directories.as_slice(),
        message_sender,
    )
    .await?;

    errors.append(&mut file_backup_errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::FileBackupErrors(errors))
    }
}

pub enum Command {
    Backup {
        source_root: std::path::PathBuf,
        destination_root: std::path::PathBuf,
    },
    Sync {
        source_root: std::path::PathBuf,
        destination_root: std::path::PathBuf,
    },
    Restore {
        source_root: std::path::PathBuf,
        destination_root: std::path::PathBuf,
        delete_files: bool,
    },
}

pub async fn run(
    commands: Command,
    message_sender: tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<(), Error> {
    match commands {
        Command::Backup {
            source_root,
            destination_root,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            backup(&source_root, &destination_root, &message_sender).await
        }
        Command::Sync {
            source_root,
            destination_root,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            backup(&source_root, &destination_root, &message_sender).await?;
            purge_files_and_dirs_in_destination(&source_root, &destination_root).await?;
            Ok(())
        }
        Command::Restore {
            source_root,
            destination_root,
            delete_files,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            // NOTE: Same as sync but switch arguments
            backup(&destination_root, &source_root, &message_sender).await?;
            if delete_files {
                purge_files_and_dirs_in_destination(&destination_root, &source_root).await?;
            }
            Ok(())
        }
    }
}

#[inline]
fn get_destination_file_path(
    destination_root: &std::path::Path,
    source_root: &std::path::Path,
    source_path: &std::path::Path,
) -> Result<std::path::PathBuf, (std::path::PathBuf, FileBackupError)> {
    let path_end = source_path.strip_prefix(source_root).map_err(|e| {
        (
            source_path.to_owned(),
            FileBackupError::CouldNotGenerateDestinationPath(e),
        )
    })?;

    Ok([destination_root, path_end].iter().collect())
}

async fn purge_files_and_dirs_in_destination<P: AsRef<std::path::Path>>(
    source_root: P,
    destination_root: P,
) -> Result<(), Error> {
    use futures::stream::StreamExt;

    let source_root = source_root.as_ref();
    let destination_root = destination_root.as_ref();
    let source_recurse_directories =
        RecursiveReadDir::try_new(source_root, ReadDirType::DirectoriesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_root.to_owned(), e))?;
    let destination_recurse_directories =
        RecursiveReadDir::try_new(destination_root, ReadDirType::DirectoriesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(destination_root.to_owned(), e))?;

    let dirs_to_delete = get_paths_in_destinatination_but_not_in_source(
        source_recurse_directories,
        destination_recurse_directories,
    )
    .await
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;

    let mut errors = vec![];
    let mut deleted_dirs = vec![];
    let mut dir_stream = futures::stream::iter(dirs_to_delete);
    while let Some(dir) = dir_stream.next().await {
        if deleted_dirs.iter().any(|d| dir.starts_with(d)) {
            continue;
        }
        if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
            errors.push((dir, FileBackupError::CannotDeleteDirectory(e)));
        } else {
            println!("Deleted \"{}\"", dir.display());
            deleted_dirs.push(dir);
        }
    }

    let source_recurse_files = RecursiveReadDir::try_new(source_root, ReadDirType::FilesOnly)
        .map_err(|e| Error::CannotReadDirectoryContent(source_root.to_owned(), e))?;
    let destination_recurse_files =
        RecursiveReadDir::try_new(destination_root, ReadDirType::FilesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(destination_root.to_owned(), e))?;

    let files_to_delete = get_paths_in_destinatination_but_not_in_source(
        source_recurse_files,
        destination_recurse_files,
    )
    .await
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;

    let mut errors_file: Vec<_> = futures::stream::iter(files_to_delete)
        .map(|file| async move {
            tokio::fs::remove_file(&file)
                .await
                .map_err(|e| (file.to_owned(), FileBackupError::CannotDeleteFile(e)))
                .map(|_| {
                    println!("Deleted \"{}\"", file.display());
                })
        })
        .buffer_unordered(cpu_count())
        .filter_map(async move |res| res.err())
        .collect()
        .await;

    if !errors.is_empty() || !errors_file.is_empty() {
        errors.append(&mut errors_file);
        Err(Error::FileBackupErrors(errors))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRONG_TEST_DIR: &str = "-------";
    const TEST_DIR: &str = "testdir";
    const TEST_DIR_LESS: &str = "testdir_less";
    const TEST_DIR_LESS_AND_ADDITIONAL: &str = "testdir_less_and_additional";
    const TEST_DIR_ADDITIONAL: &str = "testdir_additional";
    const TEST_DIR_FILES: [&str; 12] = [
        // TODO: Check if this order is the same on all platforms
        "testdir/03_a",
        "testdir/05_directory.csv",
        "testdir/04_test.py",
        "testdir/01_This.txt",
        "testdir/02_is.o",
        "testdir/more/wèirder,name.txt",
        "testdir/more/weird name.txt",
        "testdir/more2/some more file",
        "testdir/more2/some file",
        "testdir/more/even-mörer/all-solutions.bak",
        "testdir/more/even-möre/wèirder,name.txt",
        "testdir/more2/moredir/epic.file",
    ];
    const TEST_DIR_DIRECTORIES: [&str; 5] = [
        // TODO: Check if this order is the same on all platforms
        "testdir/more/",
        "testdir/more2/",
        "testdir/more/even-mörer/",
        "testdir/more/even-möre/",
        "testdir/more2/moredir/",
    ];

    #[test]
    fn test_recurse_files() {
        let file_entry = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::FilesOnly).unwrap();
        let files = file_entry
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(dbg!(&files).len(), dbg!(&TEST_DIR_FILES).len());
        assert_eq!(
            files,
            TEST_DIR_FILES
                .iter()
                .map(std::path::Path::new)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn test_recurse_directories() {
        let file_entry = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::DirectoriesOnly).unwrap();
        let files = file_entry
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(dbg!(&files).len(), dbg!(&TEST_DIR_DIRECTORIES).len());
        assert_eq!(
            files,
            TEST_DIR_DIRECTORIES
                .iter()
                .map(std::path::Path::new)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn test_recursive_readdir_fail() {
        let file_entry = RecursiveReadDir::try_new(WRONG_TEST_DIR, ReadDirType::DirectoriesOnly);
        assert!(dbg!(file_entry).is_err());
        let file_entry = RecursiveReadDir::try_new(WRONG_TEST_DIR, ReadDirType::FilesOnly);
        assert!(dbg!(file_entry).is_err());
    }

    #[test]
    fn test_get_destination_path() {
        let destination_path = get_destination_file_path(
            std::path::Path::new("destination/root"),
            std::path::Path::new("source/root"),
            std::path::Path::new("source/root/some/file.txt"),
        )
        .unwrap();
        assert_eq!(
            destination_path,
            std::path::Path::new("destination/root/some/file.txt")
        );

        let destination_path = get_destination_file_path(
            std::path::Path::new("destination/root"),
            std::path::Path::new("source/root"),
            std::path::Path::new("source/root/some/dir/"),
        )
        .unwrap();
        assert_eq!(
            destination_path,
            std::path::Path::new("destination/root/some/dir/")
        );
    }

    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_directories_less_and_additional() {
        let recurse_testdir =
            RecursiveReadDir::try_new(TEST_DIR, ReadDirType::DirectoriesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_LESS_AND_ADDITIONAL, ReadDirType::DirectoriesOnly)
                .unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        let mut difference = [
            std::path::Path::new("testdir/more2"),
            std::path::Path::new("testdir/more/even-mörer"),
            std::path::Path::new("testdir/more2/moredir"),
        ];
        difference.sort();
        assert_eq!(res, difference);
    }

    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_files_less_and_additional() {
        let recurse_testdir = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::FilesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_LESS_AND_ADDITIONAL, ReadDirType::FilesOnly)
                .unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        let mut difference = [
            std::path::Path::new("testdir/03_a"),
            std::path::Path::new("testdir/more/weird name.txt"),
            std::path::Path::new("testdir/more2/some more file"),
            std::path::Path::new("testdir/more2/some file"),
            std::path::Path::new("testdir/more/even-mörer/all-solutions.bak"),
            std::path::Path::new("testdir/more2/moredir/epic.file"),
        ];
        difference.sort();
        assert_eq!(res, difference);
    }
    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_directories_less() {
        let recurse_testdir =
            RecursiveReadDir::try_new(TEST_DIR, ReadDirType::DirectoriesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_LESS, ReadDirType::DirectoriesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        let mut difference = [
            std::path::Path::new("testdir/more2"),
            std::path::Path::new("testdir/more/even-mörer"),
            std::path::Path::new("testdir/more2/moredir"),
        ];
        difference.sort();
        assert_eq!(res, difference);
    }

    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_files_less() {
        let recurse_testdir = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::FilesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_LESS, ReadDirType::FilesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        let mut difference = [
            std::path::Path::new("testdir/03_a"),
            std::path::Path::new("testdir/more/weird name.txt"),
            std::path::Path::new("testdir/more2/some more file"),
            std::path::Path::new("testdir/more2/some file"),
            std::path::Path::new("testdir/more/even-mörer/all-solutions.bak"),
            std::path::Path::new("testdir/more2/moredir/epic.file"),
        ];
        difference.sort();
        assert_eq!(res, difference);
    }

    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_directories_additional() {
        let recurse_testdir =
            RecursiveReadDir::try_new(TEST_DIR, ReadDirType::DirectoriesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_ADDITIONAL, ReadDirType::DirectoriesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        assert!(dbg!(res).is_empty());
    }

    #[tokio::test]
    async fn test_get_paths_in_destinatination_but_not_in_source_files_additional() {
        let recurse_testdir = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::FilesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR_ADDITIONAL, ReadDirType::FilesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .await
            .unwrap();

        assert!(dbg!(res).is_empty());
    }
}
