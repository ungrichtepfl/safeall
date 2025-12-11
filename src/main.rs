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

struct RecursiveReadDir {
    next_readdirs: std::collections::VecDeque<std::path::PathBuf>,
    current_readdir: Option<std::fs::ReadDir>,
    current_dirpath: std::path::PathBuf,
}

impl From<&str> for RecursiveReadDir {
    fn from(path: &str) -> Self {
        Self::from(std::path::Path::new(path))
    }
}

impl From<&std::path::Path> for RecursiveReadDir {
    fn from(path: &std::path::Path) -> Self {
        let mut next_readdirs = std::collections::VecDeque::new();
        next_readdirs.push_back(path.into());
        Self {
            current_readdir: None,
            next_readdirs,
            current_dirpath: path.into(),
        }
    }
}

impl Iterator for RecursiveReadDir {
    type Item = Result<std::path::PathBuf, CoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        use CoreError as E;
        if let Some(current_readdir) = &mut self.current_readdir {
            for entry in current_readdir {
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
                        return Some(Err(E::CannotGetDirEntry(
                            self.current_dirpath.clone(),
                            error,
                        )));
                    }
                }
            }
        }
        // We know that the `current_readdir` is empty or is None so will create a new one
        if let Some(next_readdir) = self.next_readdirs.pop_front() {
            match std::fs::read_dir(&next_readdir) {
                Ok(readdir) => {
                    self.current_readdir = Some(readdir);
                    self.current_dirpath = next_readdir.clone();
                    return Some(Ok(next_readdir));
                }
                Err(error) => {
                    return Some(Err(E::CannotReadDirectoryContent(next_readdir, error)));
                }
            }
        }
        debug_assert!(
            self.next_readdirs.is_empty(),
            "Next readdirs must be empty."
        );
        None
    }
}

#[derive(Debug)]
enum CoreError {
    SourcePathDoesNotExist(std::path::PathBuf),
    CannotCreateDestinationDir(std::path::PathBuf, std::io::Error),
    DestinationIsNotADirectory(std::path::PathBuf),
    CannotReadDirectoryContent(std::path::PathBuf, std::io::Error),
    CannotGetDirEntry(std::path::PathBuf, std::io::Error),
    DestinationForSourceDirExistsAsFile(std::path::PathBuf, std::path::PathBuf),
    CannotCopyFile(std::path::PathBuf, std::path::PathBuf, std::io::Error),
    CouldNotGenerateDestinationPath(std::path::StripPrefixError),
}

impl std::error::Error for CoreError {}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::SourcePathDoesNotExist(path) => {
                write!(f, "Source path does not exist: {}", path.display())
            }

            CoreError::CannotCreateDestinationDir(path, error) => {
                write!(
                    f,
                    "Cannot create destination path \"{}\": {error}",
                    path.display()
                )
            }

            CoreError::DestinationIsNotADirectory(path) => {
                write!(f, "Destination is not a directory: {}", path.display())
            }

            CoreError::CannotReadDirectoryContent(path, error) => {
                write!(
                    f,
                    "Could not read source directory \"{}\": {error}",
                    path.display()
                )
            }

            CoreError::CannotGetDirEntry(path, error) => {
                write!(
                    f,
                    "Could not read source directory \"{}\": {error}",
                    path.display()
                )
            }

            CoreError::DestinationForSourceDirExistsAsFile(source_path, destination_dir) => write!(
                f,
                "Could not create a new destination directory for path \"{}\" because destination already exists but not as a directory: {}",
                source_path.display(),
                destination_dir.display()
            ),

            CoreError::CannotCopyFile(source_file, destination_file, error) => write!(
                f,
                "Could not copy source \"{}\" to destination file \"{}\": {error}",
                source_file.display(),
                destination_file.display()
            ),

            CoreError::CouldNotGenerateDestinationPath(strip_prefix_error) => write!(
                f,
                "Could not generate destination path: {strip_prefix_error}"
            ),
        }
    }
}

#[derive(Debug)]
enum Error {
    CliError(CliError),
    CoreErrors(Vec<CoreError>),
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
            Error::CoreErrors(errors) => errors.iter().try_for_each(|e| e.fmt(f)),
        }
    }
}

#[derive(Debug)]
struct Config {
    source_directory: std::path::PathBuf,
    destination_directory: std::path::PathBuf,
}

impl Config {
    fn try_from_cli() -> Result<Self, CliError> {
        let args: Vec<_> = std::env::args_os().collect();
        let [_, source_directory, destination_directory] = args
            .try_into()
            .map_err(|args: Vec<_>| CliError::WrongNumberOfArguments(args.len()))?;

        Ok(Self {
            source_directory: source_directory.into(),
            destination_directory: destination_directory.into(),
        })
    }
}

#[must_use]
fn backup_directory(
    source_directory: &std::path::Path,
    destination_directory: &std::path::Path,
) -> Vec<CoreError> {
    debug_assert!(source_directory.is_dir(), "Source is not a dir");
    debug_assert!(destination_directory.is_dir(), "Destination is not a dir");
    let mut errors = vec![];

    for source_file in RecursiveReadDir::from(source_directory) {
        match source_file {
            Ok(source_file) => {
                if let Err(err) = backup_file(source_directory, destination_directory, source_file)
                {
                    errors.push(err);
                }
            }
            Err(err) => errors.push(err),
        }
    }
    errors
}

fn backup_file(
    source_directory: &std::path::Path,
    destination_directory: &std::path::Path,
    source_file: std::path::PathBuf,
) -> Result<(), CoreError> {
    let new_destination_file =
        get_destination_file_path(destination_directory, source_directory, &source_file)?;

    if !source_file.is_dir() {
        // Is a file or a symbolic link
        return copy_if_newer(&source_file, &new_destination_file);
    }
    // It is a dir
    if !new_destination_file.exists() {
        return std::fs::create_dir(&new_destination_file)
            .map_err(|e| CoreError::CannotCreateDestinationDir(new_destination_file, e));
    }
    // It already exists
    if !new_destination_file.is_dir() {
        // If it exists it must be a directory too
        return Err(CoreError::DestinationForSourceDirExistsAsFile(
            source_file,
            new_destination_file,
        ));
    }
    Ok(())
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
) -> Result<(), CoreError> {
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
        return Ok(());
    }

    println!(
        "Copy \"{}\" to \"{}\"",
        source_file.display(),
        destination_file.display()
    );
    std::fs::copy(source_file, destination_file)
        .map_err(|e| CoreError::CannotCopyFile(source_file.into(), destination_file.into(), e))?;
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

fn run(config: Config) -> Vec<CoreError> {
    if !config.source_directory.exists() {
        return vec![CoreError::SourcePathDoesNotExist(config.source_directory)];
    }
    if !config.destination_directory.exists()
        && let Err(e) = std::fs::create_dir_all(&config.destination_directory)
    {
        return vec![CoreError::CannotCreateDestinationDir(
            config.destination_directory,
            e,
        )];
    }
    if !config.destination_directory.is_dir() {
        return vec![CoreError::DestinationIsNotADirectory(
            config.destination_directory,
        )];
    }

    backup_directory(&config.source_directory, &config.destination_directory)
}

#[inline]
fn convert_to_error(errors: Vec<CoreError>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::CoreErrors(errors))
    }
}

#[inline]
fn get_destination_file_path(
    destination_root: &std::path::Path,
    source_root: &std::path::Path,
    source_path: &std::path::Path,
) -> Result<std::path::PathBuf, CoreError> {
    let path_end = source_path
        .strip_prefix(source_root)
        .map_err(CoreError::CouldNotGenerateDestinationPath)?;

    Ok([destination_root, path_end].iter().collect())
}

fn cli() -> Result<(), Error> {
    let config = Config::try_from_cli()?;
    convert_to_error(run(config))
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
    const TEST_DIR_FILES: [&str; 16] = [
        "testdir/",
        "testdir/03_a",
        "testdir/05_directory.csv",
        "testdir/04_test.py",
        "testdir/01_This.txt",
        "testdir/02_is.o",
        "testdir/more/",
        "testdir/more/wèirder,name.txt",
        "testdir/more/weird name.txt",
        "testdir/more2/",
        "testdir/more2/some more file",
        "testdir/more2/some file",
        "testdir/more/even-möre/",
        "testdir/more/even-möre/wèirder,name.txt",
        "testdir/more2/moredir/",
        "testdir/more2/moredir/epic.file",
    ];

    #[test]
    fn test_iterate_test_dir() -> Result<(), CoreError> {
        let file_entry: RecursiveReadDir = TEST_DIR.into();
        let files: Result<Vec<_>, _> = file_entry.into_iter().collect();
        let files = files?;
        assert_eq!(files.len(), TEST_DIR_FILES.len());
        assert_eq!(
            files,
            TEST_DIR_FILES
                .iter()
                .map(std::path::Path::new)
                .collect::<Vec<_>>()
        );
        Ok(())
    }
    #[test]
    fn test_file_entry_fail() {
        let mut file_entry: RecursiveReadDir = WRONG_TEST_DIR.into();
        let next_file_entry = file_entry.next();
        assert!(next_file_entry.is_some());
        assert!(next_file_entry.unwrap().is_err());
        assert!(file_entry.next().is_none());
    }

    #[test]
    fn test_get_destination_path() -> Result<(), CoreError> {
        let destination_path = get_destination_file_path(
            std::path::Path::new("destination/root"),
            std::path::Path::new("source/root"),
            std::path::Path::new("source/root/some/file.txt"),
        )?;
        assert_eq!(
            destination_path,
            std::path::Path::new("destination/root/some/file.txt")
        );

        let destination_path = get_destination_file_path(
            std::path::Path::new("destination/root"),
            std::path::Path::new("source/root"),
            std::path::Path::new("source/root/some/dir/"),
        )?;
        assert_eq!(
            destination_path,
            std::path::Path::new("destination/root/some/dir/")
        );

        Ok(())
    }
}
