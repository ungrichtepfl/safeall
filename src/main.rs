const MAINTAINER_EMAIL: &str = "christoph.ungricht@outlook.com";

#[derive(Debug)]
enum CliError {
    WrongNumberOfArguments(usize),
}

impl std::error::Error for CliError {}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::WrongNumberOfArguments(num) => {
                if *num > 0 {
                    write!(
                        f,
                        "Wrong number of arguments. Expected 2 found {}.",
                        num - 1
                    )
                } else {
                    write!(
                        f,
                        "Somehow this OS does not pass the program name as the first argument. Contact the maintainer to fix the program for your OS: {MAINTAINER_EMAIL}."
                    )
                }
            }
        }
    }
}

struct RecurseDirectories {
    for_root: std::path::PathBuf,
    next_readdirs: std::collections::VecDeque<std::path::PathBuf>,
    current_readdir: std::fs::ReadDir,
    current_dirpath: std::path::PathBuf,
}
impl RecurseDirectories {
    fn root_directory(&self) -> &std::path::Path {
        self.for_root.as_ref()
    }
}

impl TryFrom<&str> for RecurseDirectories {
    type Error = Error;
    fn try_from(directory: &str) -> Result<Self, Self::Error> {
        Self::try_from(std::path::Path::new(directory))
    }
}
impl TryFrom<&std::path::PathBuf> for RecurseDirectories {
    type Error = Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.as_path())
    }
}

impl TryFrom<&std::path::Path> for RecurseDirectories {
    type Error = Error;

    fn try_from(directory: &std::path::Path) -> Result<Self, Self::Error> {
        let current_readdir = std::fs::read_dir(directory)
            .map_err(|e| Error::CannotReadDirectoryContent(directory.to_owned(), e))?;
        Ok(Self {
            for_root: directory.to_owned(),
            current_readdir,
            next_readdirs: std::collections::VecDeque::new(),
            current_dirpath: directory.to_owned(),
        })
    }
}

impl Iterator for RecurseDirectories {
    type Item = Result<std::path::PathBuf, (std::path::PathBuf, FileBackupError)>;

    fn next(&mut self) -> Option<Self::Item> {
        use FileBackupError as E;
        for entry in &mut self.current_readdir {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_dir() {
                        self.next_readdirs.push_back(path);
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
                    return Some(Ok(next_readdir));
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
        None
    }
}

struct RecurseFiles {
    for_root: std::path::PathBuf,
    next_readdirs: std::collections::VecDeque<std::path::PathBuf>,
    current_readdir: std::fs::ReadDir,
    current_dirpath: std::path::PathBuf,
}

impl RecurseFiles {
    fn root_directory(&self) -> &std::path::Path {
        self.for_root.as_ref()
    }
}

impl TryFrom<&str> for RecurseFiles {
    type Error = Error;
    fn try_from(directory: &str) -> Result<Self, Self::Error> {
        Self::try_from(std::path::Path::new(directory))
    }
}

impl TryFrom<&std::path::PathBuf> for RecurseFiles {
    type Error = Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.as_path())
    }
}

impl TryFrom<&std::path::Path> for RecurseFiles {
    type Error = Error;

    fn try_from(directory: &std::path::Path) -> Result<Self, Self::Error> {
        let current_readdir = std::fs::read_dir(directory)
            .map_err(|e| Error::CannotReadDirectoryContent(directory.to_owned(), e))?;
        Ok(Self {
            for_root: directory.to_owned(),
            current_readdir,
            next_readdirs: std::collections::VecDeque::new(),
            current_dirpath: directory.to_owned(),
        })
    }
}

impl Iterator for RecurseFiles {
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
                        } else {
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
                        self.current_dirpath = next_readdir;
                        continue 'drain_current_readdir;
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
enum FileBackupError {
    CannotCreateDestinationDir(std::path::PathBuf, std::io::Error),
    CannotReadDirectoryContent(std::io::Error),
    CannotGetDirEntry(std::io::Error),
    DestinationForSourceDirExistsAsFile(std::path::PathBuf),
    CannotCopyFile(std::path::PathBuf, std::io::Error),
    CouldNotGenerateDestinationPath(std::path::StripPrefixError),
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
        }
    }
}

#[derive(Debug)]
enum Error {
    CliError(CliError),
    FileBackupErrors(Vec<(std::path::PathBuf, FileBackupError)>),
    SourceRootPathDoesNotExist(std::path::PathBuf),
    CannotReadDirectoryContent(std::path::PathBuf, std::io::Error),
    CannotCreateRootDestinationDir(std::path::PathBuf, std::io::Error),
    RootDestinatinIsNotADirectory(std::path::PathBuf),
}

impl From<CliError> for Error {
    fn from(cli_error: CliError) -> Self {
        Self::CliError(cli_error)
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CliError(cli_error) => cli_error.fmt(f),
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

#[derive(Debug)]
struct Config {
    source_directory_root: std::path::PathBuf,
    destination_directory_root: std::path::PathBuf,
}

impl Config {
    fn try_from_cli() -> Result<Self, CliError> {
        let args: Vec<_> = std::env::args_os().collect();
        let [_, source_directory, destination_directory] = args
            .try_into()
            .map_err(|args: Vec<_>| CliError::WrongNumberOfArguments(args.len()))?;

        Ok(Self {
            source_directory_root: source_directory.into(),
            destination_directory_root: destination_directory.into(),
        })
    }
}

#[must_use]
fn backup_all_files(
    source_recurse_files: RecurseFiles,
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

    let mut errors = vec![];
    let source_directory_root = source_recurse_files.root_directory().to_owned();
    for source_file in source_recurse_files {
        match source_file {
            Ok(source_file) => {
                if not_existing_destination_directories
                    .iter()
                    .any(|d| source_file.starts_with(d))
                {
                    // Do not try to copy files for directories that do not exist
                    continue;
                }

                if let Err(err) = backup_single_file(
                    &source_directory_root,
                    destination_directory_root,
                    source_file,
                ) {
                    errors.push(err);
                }
            }
            Err(err) => errors.push(err),
        }
    }
    errors
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
    source_recurse_directory: RecurseDirectories,
    destination_directory_root: &std::path::Path,
) -> Vec<(std::path::PathBuf, FileBackupError)> {
    debug_assert!(
        source_recurse_directory.root_directory().is_dir(),
        "Source is not a dir"
    );
    debug_assert!(
        destination_directory_root.is_dir(),
        "Destination is not a dir"
    );

    let mut errors = vec![];
    let source_directory_root = source_recurse_directory.root_directory().to_owned();
    for source_directory in source_recurse_directory {
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
        "Destiation file must not be a directory."
    );

    let source_metadata = FileMetaData::try_new(source_file);
    if skip_copy(source_file, destination_file, &source_metadata) {
        println!(
            "Not copying \"{}\" destination \"{}\" did not change.",
            source_file.display(),
            destination_file.display()
        );
        return Ok(());
    }

    println!(
        "Copy \"{}\" to \"{}\"",
        source_file.display(),
        destination_file.display()
    );
    std::fs::copy(source_file, destination_file).map_err(|e| {
        (
            source_file.to_owned(),
            FileBackupError::CannotCopyFile(destination_file.to_owned(), e),
        )
    })?;
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

fn run(config: Config) -> Result<(), Error> {
    if !config.source_directory_root.exists() {
        return Err(Error::SourceRootPathDoesNotExist(
            config.source_directory_root,
        ));
    }
    if !config.destination_directory_root.exists()
        && let Err(e) = std::fs::create_dir_all(&config.destination_directory_root)
    {
        return Err(Error::CannotCreateRootDestinationDir(
            config.destination_directory_root,
            e,
        ));
    }
    if !config.destination_directory_root.is_dir() {
        return Err(Error::RootDestinatinIsNotADirectory(
            config.destination_directory_root,
        ));
    }

    let source_recurse_directories = RecurseDirectories::try_from(&config.source_directory_root)?;

    let mut errors = create_all_directories_in_destination(
        source_recurse_directories,
        &config.destination_directory_root,
    );
    let non_exsting_destination_directories = errors.iter().map(|(p, _)| p.as_path()).collect();

    let source_recurse_files = RecurseFiles::try_from(&config.source_directory_root)?;
    let mut file_backup_errors = backup_all_files(
        source_recurse_files,
        &config.destination_directory_root,
        non_exsting_destination_directories,
    );

    errors.append(&mut file_backup_errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::FileBackupErrors(errors))
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

fn cli() -> Result<(), Error> {
    let config = Config::try_from_cli()?;
    run(config)
}

fn main() -> std::process::ExitCode {
    if let Err(e) = cli() {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRONG_TEST_DIR: &str = "-------";
    const TEST_DIR: &str = "testdir";
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
        "testdir/more/even-mörer",
        "testdir/more/even-möre/",
        "testdir/more2/moredir/",
    ];

    #[test]
    fn test_recurse_files() {
        let file_entry = RecurseFiles::try_from(TEST_DIR).unwrap();
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
    fn test_recurse_files_fail() {
        let file_entry = RecurseFiles::try_from(WRONG_TEST_DIR);
        assert!(file_entry.is_err());
    }
    #[test]
    fn test_recurse_directories() {
        let file_entry = RecurseDirectories::try_from(TEST_DIR).unwrap();
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
    fn test_recurse_directories_fail() {
        let file_entry = RecurseDirectories::try_from(WRONG_TEST_DIR);
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
}
