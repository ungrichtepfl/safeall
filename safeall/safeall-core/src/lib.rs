pub const MAINTAINER_EMAIL: &str = "christoph.ungricht@outlook.com";

pub enum ReadDirType {
    DirectoriesOnly,
    FilesOnly,
}

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
                    "Cannot strip prefix \"{}\" from \"{}\" => {strip_prefix_error}",
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
                    "Cannot create destination path \"{}\": {error}",
                    path.display()
                )
            }
            FileBackupError::CannotReadDirectoryContent(error) => {
                write!(f, "Could not read source directory => {error}")
            }
            FileBackupError::CannotGetDirEntry(error) => {
                write!(f, "Could not read entries of directory => {error}")
            }
            FileBackupError::DestinationForSourceDirExistsAsFile(destination_dir) => write!(
                f,
                "Could not create a new destination directory \"{}\" because destination already exists but not as a directory",
                destination_dir.display()
            ),
            FileBackupError::CannotCopyFile(destination_file, error) => write!(
                f,
                "Could not copy file to destination \"{}\" => {error}",
                destination_file.display()
            ),
            FileBackupError::CouldNotGenerateDestinationPath(strip_prefix_error) => write!(
                f,
                "Could not generate destination path => {strip_prefix_error}"
            ),
            FileBackupError::InvariantBroken(error) => write!(
                f,
                "There is some problem with the program. Contact the maintainer {MAINTAINER_EMAIL} to fix the error => {error}"
            ),
            FileBackupError::CannotDeleteDirectory(error) => {
                write!(f, "Cannot delet directory => {error}")
            }
            FileBackupError::CannotDeleteFile(error) => write!(f, "Cannot delete file => {error}"),
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
                    write!(
                        f,
                        "Could not backup for path \"{}\" => {e}{end}",
                        p.display()
                    )
                })
            }
            Error::SourceRootPathDoesNotExist(path_buf) => write!(
                f,
                "The specified source directory \"{}\" does not exists",
                path_buf.display()
            ),
            Error::CannotCreateRootDestinationDir(path_buf, error) => write!(
                f,
                "Cannot create a new destination directory \"{}\" => {error}",
                path_buf.display()
            ),
            Error::RootDestinatinIsNotADirectory(path_buf) => write!(
                f,
                "Specified destination \"{}\" is not a directory but a file",
                path_buf.display()
            ),
            Error::CannotReadDirectoryContent(path_buf, error) => write!(
                f,
                "Cannot iterate through directory\"{}\" => {error}",
                path_buf.display()
            ),
        }
    }
}

#[must_use]
fn backup_all_files(
    source_recurse_files: RecursiveReadDir,
    destination_directory_root: &std::path::Path,
    not_existing_destination_directories: Vec<&std::path::Path>,
) -> Vec<(std::path::PathBuf, FileBackupError)> {
    debug_assert!(
        source_recurse_files.root_directory().is_dir(),
        "Source is not a dir"
    );
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );

    let errors = std::sync::Mutex::new(vec![]);
    let source_directory_root = source_recurse_files.root_directory().to_owned();

    use rayon::iter::ParallelBridge;
    use rayon::prelude::ParallelIterator;
    source_recurse_files.par_bridge().for_each(|source_file| {
        match source_file {
            Ok(source_file) => {
                if not_existing_destination_directories
                    .iter()
                    .any(|d| source_file.starts_with(d))
                {
                    // Do not try to copy files for directories that do not exist
                    return;
                }

                if let Err(err) = backup_single_file(
                    &source_directory_root,
                    destination_directory_root,
                    source_file,
                ) {
                    errors.lock().unwrap().push(err);
                }
            }
            Err(err) => errors.lock().unwrap().push(err),
        }
    });
    errors.into_inner().unwrap()
}

fn backup_single_file(
    source_directory_root: &std::path::Path,
    destination_directory_root: &std::path::Path,
    source_file: std::path::PathBuf,
) -> Result<(), (std::path::PathBuf, FileBackupError)> {
    debug_assert!(!source_file.is_dir(), "Must be a file or a symbolic link");

    let new_destination_file = get_destination_file_path(
        destination_directory_root,
        source_directory_root,
        &source_file,
    )?;

    copy_if_newer(&source_file, &new_destination_file)
}

#[must_use]
fn create_all_directories_in_destination(
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
                ) {
                    errors.push(err);
                }
            }
            Err(err) => errors.push(err),
        }
    }
    errors
}

fn create_single_directory(
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
        std::fs::create_dir(&new_destination_file).map_err(|e| {
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
    fn try_new(path: &std::path::Path) -> Option<Self> {
        let metadata = path.metadata().ok()?;
        let modified = metadata.modified().ok();
        Some(Self {
            modified,
            length: metadata.len(),
            file_type: metadata.file_type(),
            permissions: metadata.permissions(),
        })
    }
}

fn get_hash(path: &std::path::Path) -> Option<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path).ok()?;
    std::io::copy(&mut file, &mut hasher).ok()?;
    let res = hasher.finalize();
    Some(res)
}

fn skip_copy(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
    source_metadata: &Option<FileMetaData>,
) -> bool {
    if !destination_file.exists() {
        return false;
    }
    let destination_metadata = FileMetaData::try_new(destination_file);
    if destination_metadata.is_none() && source_metadata.is_some()
        || destination_metadata.is_some() && source_metadata.is_none()
    {
        eprintln!(
            "WARNING: We can get the metadata from one file but not the other. We just try to copy it anyway. Files: \"{}\" and \"{}\"",
            source_file.display(),
            destination_file.display()
        );
        return false;
    }

    if *source_metadata != FileMetaData::try_new(destination_file) {
        return false;
    }

    if let (Some(src_hash), Some(dest_hash)) = (get_hash(source_file), get_hash(destination_file)) {
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

fn copy_if_newer(
    source_file: &std::path::Path,
    destination_file: &std::path::Path,
) -> Result<(), (std::path::PathBuf, FileBackupError)> {
    debug_assert!(
        !source_file.is_dir(),
        "Source file must not be a directory."
    );
    debug_assert!(
        !destination_file.is_dir(),
        "Destinaion file must not be a directory."
    );

    let source_metadata = FileMetaData::try_new(source_file);
    if skip_copy(source_file, destination_file, &source_metadata) {
        println!(
            "Not copying \"{}\" as destination \"{}\" did not change.",
            source_file.display(),
            destination_file.display()
        );
        return Ok(());
    }

    std::fs::copy(source_file, destination_file).map_err(|e| {
        (
            source_file.to_owned(),
            FileBackupError::CannotCopyFile(destination_file.to_owned(), e),
        )
    })?;
    println!(
        "Copied \"{}\" to \"{}\"",
        source_file.display(),
        destination_file.display()
    );
    if set_modified_time(&source_metadata, destination_file).is_none() {
        eprintln!(
            "WARNING: Could not copy modified time from \"{}\" to destination file \"{}\"",
            source_file.display(),
            destination_file.display()
        );
    }

    Ok(())
}

fn set_modified_time(
    source_metadata: &Option<FileMetaData>,
    destination_file: &std::path::Path,
) -> Option<()> {
    if let Some(source_metadata) = source_metadata
        && let Some(modified) = source_metadata.modified
    {
        let file = std::fs::File::open(destination_file).ok()?;
        file.set_modified(modified).ok()?;
    }

    Some(())
}

fn get_paths_in_destinatination_but_not_in_source(
    mut recurse_source: RecursiveReadDir,
    mut recurse_destination: RecursiveReadDir,
) -> Result<Vec<std::path::PathBuf>, (std::path::PathBuf, FileBackupError)> {
    let mut res = vec![];
    let source_root = recurse_source.root_directory().to_owned();
    'get_new_source: for source in recurse_source.by_ref() {
        let source = source?;
        let source_stripped = source.strip_prefix(&source_root).map_err(|e| {
            (
                source.to_owned(),
                FileBackupError::InvariantBroken(InvariantError::CannotStripPrefixOfPath(
                    source_root.to_owned(),
                    source.to_owned(),
                    e,
                )),
            )
        })?;
        'get_new_destination: loop {
            if let Some(destination) = recurse_destination.next() {
                let destination = destination?;
                let destination_stripped = destination
                    .strip_prefix(recurse_destination.root_directory())
                    .map_err(|e| {
                        (
                            destination.to_owned(),
                            FileBackupError::InvariantBroken(
                                InvariantError::CannotStripPrefixOfPath(
                                    recurse_destination.root_directory().to_owned(),
                                    destination.to_owned(),
                                    e,
                                ),
                            ),
                        )
                    })?;
                if source_stripped != destination_stripped {
                    res.push(destination);
                    continue 'get_new_destination;
                } else {
                    continue 'get_new_source;
                }
            } else {
                // No destination files left, break out
                break 'get_new_source;
            }
        }
    }

    for destination in recurse_destination {
        // If there are some destination files left they are all not in source:
        debug_assert!(
            recurse_source.next().is_none(),
            "recurse_source must be empty."
        );
        let destination = destination?;
        res.push(destination);
    }
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
fn backup<P: AsRef<std::path::Path>>(
    source_directory_root: P,
    destination_directory_root: P,
) -> Result<(), Error> {
    let source_directory_root = source_directory_root.as_ref();
    let destination_directory_root = destination_directory_root.as_ref();

    let source_recurse_directories =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::DirectoriesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e))?;

    let mut errors = create_all_directories_in_destination(
        source_recurse_directories,
        destination_directory_root,
    );
    let non_exsting_destination_directories = errors.iter().map(|(p, _)| p.as_path()).collect();

    let source_recurse_files =
        RecursiveReadDir::try_new(source_directory_root, ReadDirType::FilesOnly)
            .map_err(|e| Error::CannotReadDirectoryContent(source_directory_root.to_owned(), e))?;
    let mut file_backup_errors = backup_all_files(
        source_recurse_files,
        destination_directory_root,
        non_exsting_destination_directories,
    );

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

pub fn run(commands: Command) -> Result<(), Error> {
    match commands {
        Command::Backup {
            source_root,
            destination_root,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            backup(&source_root, &destination_root)
        }
        Command::Sync {
            source_root,
            destination_root,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            backup(&source_root, &destination_root)?;
            purge_files_and_dirs_in_destination(&source_root, &destination_root)?;
            Ok(())
        }
        Command::Restore {
            source_root,
            destination_root,
            delete_files,
        } => {
            validate_root_paths(&source_root, &destination_root)?;
            // NOTE: Same as sync but switch arguments
            backup(&destination_root, &source_root)?;
            if delete_files {
                purge_files_and_dirs_in_destination(&destination_root, &source_root)?;
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

fn purge_files_and_dirs_in_destination<P: AsRef<std::path::Path>>(
    source_root: P,
    destination_root: P,
) -> Result<(), Error> {
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
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;

    let mut errors = vec![];
    let mut deleted_dirs = vec![];
    for dir in dirs_to_delete {
        if deleted_dirs.iter().any(|d| dir.starts_with(d)) {
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&dir) {
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
    .map_err(|e| Error::FileBackupErrors(vec![e]))?;

    use rayon::iter::IntoParallelIterator;
    use rayon::prelude::ParallelIterator;
    let errors_files = std::sync::Mutex::new(vec![]);
    files_to_delete.into_par_iter().for_each(|file| {
        if let Err(e) = std::fs::remove_file(&file) {
            errors_files
                .lock()
                .unwrap()
                .push((file, FileBackupError::CannotDeleteFile(e)));
        } else {
            println!("Deleted \"{}\"", file.display());
        }
    });
    let mut errors_file = errors_files.into_inner().unwrap();
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
    const TEST_DIR2: &str = "testdir2";
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
        assert_eq!(files.len(), TEST_DIR_FILES.len());
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
        assert_eq!(files.len(), TEST_DIR_DIRECTORIES.len());
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
        assert!(file_entry.is_err());
        let file_entry = RecursiveReadDir::try_new(WRONG_TEST_DIR, ReadDirType::FilesOnly);
        assert!(file_entry.is_err());
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

    #[test]
    fn test_get_paths_in_destinatination_but_not_in_source_directories() {
        let recurse_testdir =
            RecursiveReadDir::try_new(TEST_DIR, ReadDirType::DirectoriesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR2, ReadDirType::DirectoriesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .unwrap();

        let difference = [
            std::path::Path::new("testdir/more2"),
            std::path::Path::new("testdir/more/even-mörer"),
            std::path::Path::new("testdir/more2/moredir"),
        ];
        assert_eq!(res, difference);
    }

    #[test]
    fn test_get_paths_in_destinatination_but_not_in_source_files() {
        let recurse_testdir = RecursiveReadDir::try_new(TEST_DIR, ReadDirType::FilesOnly).unwrap();
        let recurse_testdir2 =
            RecursiveReadDir::try_new(TEST_DIR2, ReadDirType::FilesOnly).unwrap();
        let res = get_paths_in_destinatination_but_not_in_source(recurse_testdir2, recurse_testdir)
            .unwrap();

        let difference = [
            std::path::Path::new("testdir/03_a"),
            std::path::Path::new("testdir/more/weird name.txt"),
            std::path::Path::new("testdir/more2/some more file"),
            std::path::Path::new("testdir/more2/some file"),
            std::path::Path::new("testdir/more/even-mörer/all-solutions.bak"),
            std::path::Path::new("testdir/more2/moredir/epic.file"),
        ];
        assert_eq!(res, difference);
    }
}
