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

pub trait MessageSender {
    fn send(&self, message: Message);
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
    type Item = Result<std::path::PathBuf, ProcessPathError>;

    fn next(&mut self) -> Option<Self::Item> {
        use FileBackupErrorKind as K;
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
                        return Some(Err(ProcessPathError {
                            not_processed: None,
                            kind: K::CannotGetDirEntry {
                                in_dir: self.current_dirpath.clone(),
                                io_error: error.to_string(),
                            },
                        }));
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
                        return Some(Err(ProcessPathError {
                            not_processed: Some(next_readdir),
                            kind: K::CannotReadDirectoryContent {
                                io_error: error.to_string(),
                            },
                        }));
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

#[derive(Debug, Clone)]
pub enum InvariantError {
    CannotStripPrefixOfPath {
        path_root: std::path::PathBuf,
        path: std::path::PathBuf,
        error: std::path::StripPrefixError,
    },
}

impl std::error::Error for InvariantError {}

impl std::fmt::Display for InvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvariantError::CannotStripPrefixOfPath {
                path_root,
                path,
                error,
            } => {
                write!(
                    f,
                    "cannot strip prefix \"{}\" from \"{}\" => {error}",
                    path_root.display(),
                    path.display()
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessPathError {
    pub not_processed: Option<std::path::PathBuf>,
    pub kind: FileBackupErrorKind,
}

#[derive(Debug, Clone)]
pub enum FileBackupErrorKind {
    CannotCreateDestinationDir {
        destination: std::path::PathBuf,
        io_error: String,
    },
    CannotReadDirectoryContent {
        io_error: String,
    },
    CannotGetDirEntry {
        in_dir: std::path::PathBuf,
        io_error: String,
    },
    DestinationForSourceDirExistsAsFile {
        destination: std::path::PathBuf,
    },
    CannotCopyFile {
        to: std::path::PathBuf,
        io_error: String,
    },
    InvariantBroken(InvariantError),
    CannotDeleteDirectory {
        io_error: String,
    },
    CannotDeleteFile {
        io_error: String,
    },
}

impl std::error::Error for ProcessPathError {}

impl std::fmt::Display for ProcessPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use FileBackupErrorKind as K;
        let prefix = if let Some(ref not_processed) = self.not_processed {
            format!("Could not process \"{}\". ", not_processed.display())
        } else {
            String::new()
        };
        match &self.kind {
            K::CannotCreateDestinationDir {
                destination,
                io_error,
            } => {
                write!(
                    f,
                    "{prefix}Cannot create destination path \"{}\": {io_error}.",
                    destination.display(),
                )
            }
            K::CannotReadDirectoryContent { io_error } => {
                write!(f, "{prefix}Could not read directory: {io_error}.")
            }
            K::CannotGetDirEntry { in_dir, io_error } => {
                write!(
                    f,
                    "{prefix}Could not read entries of directory \"{}\": {io_error}.",
                    in_dir.display()
                )
            }
            K::DestinationForSourceDirExistsAsFile { destination } => write!(
                f,
                "{prefix}Could not create a new destination directory \"{}\" because the destination already exists but not as a directory.",
                destination.display(),
            ),
            K::CannotCopyFile { to, io_error } => write!(
                f,
                "{prefix}Could not copy file to \"{}\". {io_error}.",
                to.display()
            ),
            K::InvariantBroken(invariant_error) => write!(
                f,
                "{prefix}There is some problem with the program. Contact the maintainer {MAINTAINER_EMAIL} to fix the error. Error: \"{invariant_error}\""
            ),
            K::CannotDeleteDirectory { io_error } => {
                write!(f, "{prefix}Cannot delet directory: {io_error}.")
            }
            K::CannotDeleteFile { io_error } => {
                write!(f, "{prefix}Cannot delete file: {io_error}.")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    FileBackupErrors(Vec<ProcessPathError>),
    SourceRootPathDoesNotExist(std::path::PathBuf),
    CannotReadDirectoryContent(std::path::PathBuf, String),
    CannotCreateRootDestinationDir(std::path::PathBuf, String),
    RootDestinatinIsNotADirectory(std::path::PathBuf),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::FileBackupErrors(errors) => errors.iter().enumerate().try_for_each(|(i, e)| {
                let end = if i == errors.len() - 1 { "" } else { "\n" };
                write!(f, "{e}{end}")
            }),
            Error::SourceRootPathDoesNotExist(path) => write!(
                f,
                "The specified source directory \"{}\" does not exists.",
                path.display()
            ),
            Error::CannotCreateRootDestinationDir(path_buf, io_error) => write!(
                f,
                "Cannot create a new destination directory \"{}\": {io_error}.",
                path_buf.display()
            ),
            Error::RootDestinatinIsNotADirectory(path) => write!(
                f,
                "Specified destination \"{}\" is not a directory but a file.",
                path.display()
            ),
            Error::CannotReadDirectoryContent(path, error) => write!(
                f,
                "Cannot iterate through directory\"{}\": {error}.",
                path.display()
            ),
        }
    }
}

async fn backup_all_files(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    not_existing_destination_directories: &[&std::path::PathBuf],
    message_sender: &impl MessageSender,
) -> Result<Vec<ProcessPathError>, Error> {
    debug_assert!(source_directory_root.is_dir(), "Source is not a dir");
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );
    let source_recurse_files =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::FilesOnly).map_err(|e| {
            Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e.to_string())
        })?;

    use futures::stream::StreamExt;
    let num_files = futures::stream::iter(source_recurse_files).count().await;
    message_sender.send(Message::Progress(Progress::Start(
        num_files,
        ProgressType::CopingFiles,
    )));
    let source_recurse_files =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::FilesOnly).map_err(|e| {
            Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e.to_string())
        })?;
    let errors: Vec<_> = futures::stream::iter(source_recurse_files)
        .map(async |source_file| {
            let source_file = source_file?;
            if not_existing_destination_directories
                .iter()
                .any(|d| source_file.starts_with(d))
            {
                // Do not try to copy files for directories that do not exist
                // TODO: Return error such that we know which files did not get backed up.
                return Ok(());
            }
            backup_single_file(
                source_directory_root,
                destination_directory_root,
                source_file,
                message_sender,
            )
            .await
        })
        .buffer_unordered(cpu_count())
        .filter_map(async move |res| res.err())
        .collect()
        .await;
    message_sender.send(Message::Progress(Progress::End(ProgressType::CopingFiles)));
    Ok(errors)
}

async fn backup_single_file(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    source_file: std::path::PathBuf,
    message_sender: &impl MessageSender,
) -> Result<(), ProcessPathError> {
    debug_assert!(!source_file.is_dir(), "Must be a file or a symbolic link");

    let new_destination_file = get_destination_file_path(
        destination_directory_root,
        source_directory_root,
        &source_file,
    )?;

    copy_or_skip_if_same(&source_file, &new_destination_file, message_sender).await
}

async fn create_all_directories_in_destination(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    message_sender: &impl MessageSender,
) -> Result<Vec<ProcessPathError>, Error> {
    debug_assert!(source_directory_root.is_dir(), "Source is not a dir");
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );
    let source_recurse_directories =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::DirectoriesOnly).map_err(
            |e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e.to_string()),
        )?;

    use futures::StreamExt;
    let num_dirs = futures::stream::iter(source_recurse_directories)
        .count()
        .await;
    message_sender.send(Message::Progress(Progress::Start(
        num_dirs,
        ProgressType::CreatingDirectories,
    )));

    let source_recurse_directories =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::DirectoriesOnly).map_err(
            |e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e.to_string()),
        )?;
    let mut source_stream = futures::stream::iter(source_recurse_directories);
    let mut errors = vec![];
    while let Some(source_directory) = source_stream.next().await {
        match source_directory {
            Ok(source_directory) => {
                if errors.iter().any(|e: &ProcessPathError| {
                    e.not_processed
                        .as_ref()
                        .is_some_and(|p| source_directory.starts_with(p))
                }) {
                    continue;
                }
                if let Err(err) = create_single_directory_if_not_exists(
                    source_directory_root,
                    destination_directory_root,
                    source_directory,
                    message_sender,
                )
                .await
                {
                    errors.push(err);
                }
            }
            Err(err) => errors.push(err),
        }
    }
    message_sender.send(Message::Progress(Progress::End(
        ProgressType::CreatingDirectories,
    )));
    Ok(errors)
}

async fn create_single_directory_if_not_exists(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    source_directory: std::path::PathBuf,
    message_sender: &impl MessageSender,
) -> Result<(), ProcessPathError> {
    debug_assert!(source_directory.is_dir(), "Must be a directory");
    let new_destination_dir = get_destination_file_path(
        destination_directory_root,
        source_directory_root,
        &source_directory,
    )?;

    if new_destination_dir.exists() {
        // If it already exists it must be a directory too
        if new_destination_dir.is_dir() {
            message_sender.send(Message::Progress(Progress::Increment(
                Increment::DestinationDirAlreadyExists {
                    source: source_directory.clone(),
                    destination: new_destination_dir.clone(),
                },
            )));
            Ok(())
        } else {
            Err(ProcessPathError {
                not_processed: Some(source_directory.clone()),
                kind: FileBackupErrorKind::DestinationForSourceDirExistsAsFile {
                    destination: new_destination_dir,
                },
            })
        }
    } else {
        message_sender.send(Message::Info(Info::StartCreatingDir {
            source: source_directory.clone(),
            destination: new_destination_dir.clone(),
        }));
        tokio::fs::create_dir(&new_destination_dir)
            .await
            .map_err(|e| ProcessPathError {
                not_processed: Some(source_directory.clone()),
                kind: FileBackupErrorKind::CannotCreateDestinationDir {
                    destination: new_destination_dir.clone(),
                    io_error: e.to_string(),
                },
            })
            .inspect(|_| {
                message_sender.send(Message::Progress(Progress::Increment(
                    Increment::DirCreated {
                        source: source_directory,
                        destination: new_destination_dir,
                    },
                )));
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

#[derive(Debug)]
pub enum ProgressType {
    CreatingDirectories,
    CopingFiles,
    DeletingDirs,
    DeletingFiles,
}

#[derive(Debug)]
pub enum Increment {
    SkippingFileNoModification {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    FileCopied {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    DirCreated {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    DestinationDirAlreadyExists {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    DeletedFile(std::path::PathBuf),
    DeletedDir(std::path::PathBuf),
    DirectoryAlreadyDeleted(std::path::PathBuf),
}

#[derive(Debug)]
pub enum ProgressEnd {
    FileCopied {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
}

#[derive(Debug)]
pub enum Progress {
    Start(usize, ProgressType),
    Increment(Increment),
    End(ProgressType),
}

impl std::fmt::Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Progress::Start(total, progress_type) => match progress_type {
                ProgressType::CreatingDirectories => {
                    write!(f, "Start creating {total} directories.")
                }
                ProgressType::CopingFiles => write!(f, "Start coping {total} files."),
                ProgressType::DeletingDirs => write!(f, "Start deleting {total} directories."),
                ProgressType::DeletingFiles => write!(f, "Start deleting {total} files."),
            },
            Progress::End(progress_type) => match progress_type {
                ProgressType::CreatingDirectories => write!(f, "Finished creating directories."),
                ProgressType::CopingFiles => write!(f, "Finished coping files."),
                ProgressType::DeletingDirs => write!(f, "Finished deleting directories."),
                ProgressType::DeletingFiles => write!(f, "Finished deleting files."),
            },
            Progress::Increment(increment) => match increment {
                Increment::SkippingFileNoModification {
                    source,
                    destination,
                } => write!(
                    f,
                    "Not coping \"{}\" because \"{}\" is up to date.",
                    source.display(),
                    destination.display()
                ),
                Increment::FileCopied {
                    source,
                    destination,
                } => write!(
                    f,
                    "Copied \"{}\" to \"{}\".",
                    source.display(),
                    destination.display()
                ),
                Increment::DirCreated {
                    source,
                    destination,
                } => write!(
                    f,
                    "Created directory \"{}\" to backup \"{}\".",
                    destination.display(),
                    source.display()
                ),
                Increment::DestinationDirAlreadyExists {
                    source,
                    destination,
                } => write!(
                    f,
                    "Directory \"{}\" already exists for backing up \"{}\".",
                    destination.display(),
                    source.display()
                ),
                Increment::DeletedFile(path) => write!(f, "Deleted file \"{}\".", path.display()),
                Increment::DeletedDir(path) => {
                    write!(f, "Deleted directory \"{}\".", path.display())
                }
                Increment::DirectoryAlreadyDeleted(path) => write!(
                    f,
                    "Directory \"{}\" has already been deleted.",
                    path.display()
                ),
            },
        }
    }
}

#[derive(Debug)]
pub enum Info {
    CreatingDestinationDir(std::path::PathBuf),
    DestinationDirCreated(std::path::PathBuf),
    StartCopingFile {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    StartDeletingDir(std::path::PathBuf),
    StartDeletingFile(std::path::PathBuf),
    StartCreatingDir {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
}

impl std::fmt::Display for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Info::CreatingDestinationDir(path) => {
                write!(f, "Creating directory \"{}\".", path.display())
            }
            Info::DestinationDirCreated(path) => {
                write!(f, "Directory \"{}\" created.", path.display())
            }
            Info::StartCopingFile {
                source,
                destination,
            } => write!(
                f,
                "Start coping file \"{}\" to \"{}\".",
                source.display(),
                destination.display()
            ),
            Info::StartDeletingDir(path) => {
                write!(f, "Start deleting directory \"{}\".", path.display())
            }
            Info::StartDeletingFile(path) => {
                write!(f, "Start deleting file \"{}\".", path.display())
            }
            Info::StartCreatingDir {
                source,
                destination,
            } => write!(
                f,
                "Start creating \"{}\" for \"{}\"",
                destination.display(),
                source.display()
            ),
        }
    }
}

#[derive(Debug)]
pub enum Warning {
    CannotGetMetadata {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
        copy_anyway: bool,
    },
    CannotGetHash {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
        copy_anyway: bool,
    },
    CannotCopyModifiedTime {
        source: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Warning::CannotGetMetadata {
                source,
                destination,
                copy_anyway,
            } => {
                let copy = if *copy_anyway {
                    " We try to copy the file anyway."
                } else {
                    " We do not try to copy the file."
                };

                write!(
                    f,
                    "Cannot get meta data for either \"{}\" or \"{}\".{copy}",
                    source.display(),
                    destination.display()
                )
            }
            Warning::CannotGetHash {
                source,
                destination,
                copy_anyway,
            } => {
                let copy = if *copy_anyway {
                    " We try to copy the file anyway."
                } else {
                    " We do not try to copy the file."
                };
                write!(
                    f,
                    "Cannot get the hash for \"{}\" or \"{}\".{copy}",
                    source.display(),
                    destination.display()
                )
            }
            Warning::CannotCopyModifiedTime {
                source,
                destination,
            } => write!(
                f,
                "Cannot copy the modification time from \"{}\" to \"{}\".",
                source.display(),
                destination.display()
            ),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    // TODO: Maybe we should send the errors here?
    Warning(Warning),
    Info(Info),
    Progress(Progress),
    Error(ProcessPathError),
}

async fn skip_copy(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
    source_metadata: &Option<FileMetaData>,
    message_sender: &impl MessageSender,
) -> bool {
    if !destination_file.exists() {
        return false;
    }
    let destination_metadata = FileMetaData::try_new(destination_file).await;
    if destination_metadata.is_none() && source_metadata.is_some()
        || destination_metadata.is_some() && source_metadata.is_none()
    {
        message_sender.send(Message::Warning(Warning::CannotGetMetadata {
            source: source_file.to_owned(),
            destination: destination_file.to_owned(),
            copy_anyway: true,
        }));
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
        message_sender.send(Message::Warning(Warning::CannotGetHash {
            source: source_file.to_owned(),
            destination: destination_file.to_owned(),
            copy_anyway: true,
        }));
    }

    true
}

async fn copy_or_skip_if_same(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
    message_sender: &impl MessageSender,
) -> Result<(), ProcessPathError> {
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
        message_sender.send(Message::Progress(Progress::Increment(
            Increment::SkippingFileNoModification {
                source: source_file.to_owned(),
                destination: destination_file.to_owned(),
            },
        )));
        return Ok(());
    }

    message_sender.send(Message::Info(Info::StartCopingFile {
        source: source_file.to_owned(),
        destination: destination_file.to_owned(),
    }));
    tokio::fs::copy(source_file, destination_file)
        .await
        .map_err(|e| ProcessPathError {
            not_processed: Some(source_file.to_owned()),
            kind: FileBackupErrorKind::CannotCopyFile {
                to: destination_file.to_owned(),
                io_error: e.to_string(),
            },
        })
        .inspect(|_| {
            message_sender.send(Message::Progress(Progress::Increment(
                Increment::FileCopied {
                    source: source_file.to_owned(),
                    destination: destination_file.to_owned(),
                },
            )));
        })?;

    if set_modified_time(&source_metadata, destination_file)
        .await
        .is_none()
    {
        message_sender.send(Message::Warning(Warning::CannotCopyModifiedTime {
            source: source_file.to_owned(),
            destination: destination_file.to_owned(),
        }));
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
) -> Result<Vec<std::path::PathBuf>, ProcessPathError> {
    let source_root_path = recurse_source.root_directory().to_owned();
    let source_root_path = &source_root_path;
    let destination_root_path = recurse_destination.root_directory().to_owned();
    let destination_root_path = &destination_root_path;
    use futures::stream::TryStreamExt;
    let source_files: std::collections::HashSet<_> = futures::stream::iter(recurse_source)
        .and_then(async |path| {
            path.strip_prefix(source_root_path)
                .map(|p| p.to_owned())
                .map_err(|e| ProcessPathError {
                    not_processed: Some(path.clone()),
                    kind: FileBackupErrorKind::InvariantBroken(
                        InvariantError::CannotStripPrefixOfPath {
                            path_root: source_root_path.to_owned(),
                            path: path.clone(),
                            error: e,
                        },
                    ),
                })
        })
        .try_collect()
        .await?;

    let destination_files: std::collections::HashSet<_> =
        futures::stream::iter(recurse_destination)
            .and_then(async |path| {
                path.strip_prefix(destination_root_path)
                    .map(|p| p.to_owned())
                    .map_err(|e| ProcessPathError {
                        not_processed: Some(path.to_owned()),
                        kind: FileBackupErrorKind::InvariantBroken(
                            InvariantError::CannotStripPrefixOfPath {
                                path_root: destination_root_path.clone(),
                                path: path.clone(),
                                error: e,
                            },
                        ),
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
fn validate_or_create_root_paths<P: AsRef<std::path::Path>>(
    source_directory_root: P,
    destination_directory_root: P,
    message_sender: &impl MessageSender,
) -> Result<(), Error> {
    let source_directory_root = source_directory_root.as_ref();
    let destination_directory_root = destination_directory_root.as_ref();

    if !source_directory_root.exists() {
        return Err(Error::SourceRootPathDoesNotExist(
            source_directory_root.to_owned(),
        ));
    }
    if !destination_directory_root.exists() {
        message_sender.send(Message::Info(Info::CreatingDestinationDir(
            destination_directory_root.to_owned(),
        )));
        std::fs::create_dir_all(destination_directory_root)
            .map_err(|e| {
                Error::CannotCreateRootDestinationDir(
                    destination_directory_root.to_owned(),
                    e.to_string(),
                )
            })
            .inspect(|_| {
                message_sender.send(Message::Info(Info::DestinationDirCreated(
                    destination_directory_root.to_owned(),
                )));
            })?;
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
    message_sender: &impl MessageSender,
) -> Result<(), Error> {
    let source_directory_root = source_directory_root.as_ref();
    let destination_directory_root = destination_directory_root.as_ref();

    let mut errors = create_all_directories_in_destination(
        source_directory_root,
        destination_directory_root,
        message_sender,
    )
    .await?;
    let non_exsting_destination_directories: Vec<_> = errors
        .iter()
        .filter_map(|e| e.not_processed.as_ref())
        .collect();

    let mut file_backup_errors = backup_all_files(
        source_directory_root,
        destination_directory_root,
        &non_exsting_destination_directories,
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

pub async fn run(commands: Command, message_sender: impl MessageSender) -> Result<(), Error> {
    match commands {
        Command::Backup {
            source_root,
            destination_root,
        } => {
            validate_or_create_root_paths(&source_root, &destination_root, &message_sender)?;
            backup(&source_root, &destination_root, &message_sender).await
        }
        Command::Sync {
            source_root,
            destination_root,
        } => {
            validate_or_create_root_paths(&source_root, &destination_root, &message_sender)?;
            backup(&source_root, &destination_root, &message_sender).await?;
            purge_files_and_dirs_in_destination(&source_root, &destination_root, &message_sender)
                .await?;
            Ok(())
        }
        Command::Restore {
            source_root,
            destination_root,
            delete_files,
        } => {
            validate_or_create_root_paths(&source_root, &destination_root, &message_sender)?;
            // NOTE: Same as sync but switch arguments
            backup(&destination_root, &source_root, &message_sender).await?;
            if delete_files {
                purge_files_and_dirs_in_destination(
                    &destination_root,
                    &source_root,
                    &message_sender,
                )
                .await?;
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
) -> Result<std::path::PathBuf, ProcessPathError> {
    let path_end = source_path
        .strip_prefix(source_root)
        .map_err(|e| ProcessPathError {
            not_processed: Some(source_path.to_owned()),
            kind: FileBackupErrorKind::InvariantBroken(InvariantError::CannotStripPrefixOfPath {
                path_root: source_root.to_owned(),
                path: source_path.to_owned(),
                error: e,
            }),
        })?;

    Ok([destination_root, path_end].iter().collect())
}

async fn purge_files_and_dirs_in_destination<P: AsRef<std::path::Path>>(
    source_root: P,
    destination_root: P,
    message_sender: &impl MessageSender,
) -> Result<(), Error> {
    use futures::stream::StreamExt;

    let source_root = source_root.as_ref();
    let destination_root = destination_root.as_ref();
    let source_recurse_directories =
        RecursiveReadDir::try_new(source_root, ReadDirType::DirectoriesOnly).map_err(|e| {
            Error::CannotReadDirectoryContent(source_root.to_owned(), e.to_string())
        })?;
    let destination_recurse_directories =
        RecursiveReadDir::try_new(destination_root, ReadDirType::DirectoriesOnly).map_err(|e| {
            Error::CannotReadDirectoryContent(destination_root.to_owned(), e.to_string())
        })?;

    let dirs_to_delete = get_paths_in_destinatination_but_not_in_source(
        source_recurse_directories,
        destination_recurse_directories,
    )
    .await
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;

    let num_dirs = futures::stream::iter(dirs_to_delete.clone()).count().await;
    message_sender.send(Message::Progress(Progress::Start(
        num_dirs,
        ProgressType::DeletingDirs,
    )));

    let mut errors = vec![];
    let mut deleted_dirs = vec![];
    let mut dir_stream = futures::stream::iter(dirs_to_delete);
    while let Some(dir) = dir_stream.next().await {
        if deleted_dirs.iter().any(|d| dir.starts_with(d)) {
            message_sender.send(Message::Progress(Progress::Increment(
                Increment::DirectoryAlreadyDeleted(dir),
            )));
            continue;
        }
        message_sender.send(Message::Info(Info::StartDeletingDir(dir.clone())));
        if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
            let error = ProcessPathError {
                not_processed: Some(dir),
                kind: FileBackupErrorKind::CannotDeleteDirectory {
                    io_error: e.to_string(),
                },
            };
            errors.push(error.clone());
        } else {
            message_sender.send(Message::Progress(Progress::Increment(
                Increment::DeletedDir(dir.clone()),
            )));
            deleted_dirs.push(dir);
        }
    }
    message_sender.send(Message::Progress(Progress::End(ProgressType::DeletingDirs)));

    let source_recurse_files = RecursiveReadDir::try_new(source_root, ReadDirType::FilesOnly)
        .map_err(|e| Error::CannotReadDirectoryContent(source_root.to_owned(), e.to_string()))?;
    let destination_recurse_files =
        RecursiveReadDir::try_new(destination_root, ReadDirType::FilesOnly).map_err(|e| {
            Error::CannotReadDirectoryContent(destination_root.to_owned(), e.to_string())
        })?;

    let files_to_delete = get_paths_in_destinatination_but_not_in_source(
        source_recurse_files,
        destination_recurse_files,
    )
    .await
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;
    let num_files = futures::stream::iter(files_to_delete.clone()).count().await;
    message_sender.send(Message::Progress(Progress::Start(
        num_files,
        ProgressType::DeletingFiles,
    )));

    let mut errors_file: Vec<_> = futures::stream::iter(files_to_delete)
        .map(async |file| {
            message_sender.send(Message::Info(Info::StartDeletingFile(file.clone())));
            tokio::fs::remove_file(&file)
                .await
                .map_err(|e| ProcessPathError {
                    not_processed: Some(file.to_owned()),
                    kind: FileBackupErrorKind::CannotDeleteFile {
                        io_error: e.to_string(),
                    },
                })
                .inspect(|_| {
                    message_sender.send(Message::Progress(Progress::Increment(
                        Increment::DeletedFile(file),
                    )));
                })
        })
        .buffer_unordered(cpu_count())
        .filter_map(async move |res| res.err())
        .collect()
        .await;
    message_sender.send(Message::Progress(Progress::End(
        ProgressType::DeletingFiles,
    )));

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
