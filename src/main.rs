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

struct FileEntry {
    next_readdirs: std::collections::VecDeque<std::path::PathBuf>,
    current_readdir: Option<std::fs::ReadDir>,
    current_dirpath: std::path::PathBuf,
}

impl From<&str> for FileEntry {
    fn from(path: &str) -> Self {
        Self::from(std::path::Path::new(path))
    }
}

impl From<&std::path::Path> for FileEntry {
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

impl Iterator for FileEntry {
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
    CoreErrors(std::vec::Vec<CoreError>),
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
    source: std::path::PathBuf,
    destination: std::path::PathBuf,
}

impl Config {
    fn try_from_cli() -> Result<Self, CliError> {
        let args: Vec<_> = std::env::args_os().collect();
        let [_, source, destination] = args
            .try_into()
            .map_err(|args: Vec<_>| CliError::WrongNumberOfArguments(args.len()))?;

        Ok(Self {
            source: source.into(),
            destination: destination.into(),
        })
    }
}

#[must_use]
fn process_dir(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::vec::Vec<CoreError> {
    debug_assert!(source.is_dir(), "Source is not a dir");
    debug_assert!(destination.is_dir(), "Destination is not a dir");
    let mut errors = vec![];

    for path in FileEntry::from(source) {
        match path {
            Ok(path) => {
                match get_destination_path(destination, source, &path) {
                    Ok(new_destination) => {
                        if path.is_dir() {
                            if new_destination.exists() {
                                if !new_destination.is_dir() {
                                    errors.push(CoreError::DestinationForSourceDirExistsAsFile(
                                        path,
                                        new_destination,
                                    ));
                                }
                            } else if let Err(e) = std::fs::create_dir(&new_destination) {
                                errors.push(CoreError::CannotCreateDestinationDir(
                                    new_destination,
                                    e,
                                ));
                            }
                        } else {
                            // Is a file or a symbolic link
                            if let Err(e) = copy_if_newer(&path, &new_destination) {
                                errors.push(e);
                            }
                        }
                    }
                    Err(error) => {
                        errors.push(error);
                    }
                }
            }
            Err(err) => errors.push(err),
        }
    }
    errors
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

    let src_metadata = FileMetaData::try_new(source_file);

    if destination_file.exists() {
        if let (Some(src_metadata), Some(dest_metadata)) =
            (&src_metadata, FileMetaData::try_new(destination_file))
        {
            if *src_metadata == dest_metadata {
                if let (Some(src_hash), Some(dest_hash)) =
                    (get_hash(source_file), get_hash(destination_file))
                {
                    if src_hash == dest_hash {
                        println!(
                            "Skipping \"{}\" as destination file \"{}\" is still up to date. ",
                            source_file.display(),
                            destination_file.display()
                        );
                        return Ok(());
                    }
                } else {
                    eprintln!(
                        "WARNING: Cannot get hash of either {} or {}. We just try to copy the file anyway.",
                        source_file.display(),
                        destination_file.display()
                    );
                }
            }
        } else {
            eprintln!(
                "WARNING: Cannot read all metadata of either {} or {}. We just try to copy the file anyway.",
                source_file.display(),
                destination_file.display()
            );
        }
    }

    println!(
        "Copy \"{}\" to \"{}\"",
        source_file.display(),
        destination_file.display()
    );
    if let Err(e) = std::fs::copy(source_file, destination_file) {
        return Err(CoreError::CannotCopyFile(
            source_file.into(),
            destination_file.into(),
            e,
        ));
    }
    if let Some(src_metadata) = src_metadata
        && set_modified(&src_metadata, destination_file).is_none()
    {
        eprintln!(
            "WARNING: Could not copy modified time from \"{}\" destination file \"{}\"",
            source_file.display(),
            destination_file.display()
        );
    }

    Ok(())
}

fn set_modified(src_metadata: &FileMetaData, dest_path: &std::path::Path) -> Option<()> {
    if let Some(modified) = src_metadata.modified {
        let file = std::fs::File::open(dest_path).ok()?;
        file.set_modified(modified).ok()?;
    }

    Some(())
}

fn run(config: Config) -> std::vec::Vec<CoreError> {
    if !config.source.exists() {
        return vec![CoreError::SourcePathDoesNotExist(config.source)];
    }
    if !config.destination.exists()
        && let Err(e) = std::fs::create_dir_all(&config.destination)
    {
        return vec![CoreError::CannotCreateDestinationDir(config.destination, e)];
    }
    if !config.destination.is_dir() {
        return vec![CoreError::DestinationIsNotADirectory(config.destination)];
    }

    process_dir(&config.source, &config.destination)
}

#[inline]
fn convert_to_error(errors: std::vec::Vec<CoreError>) -> Result<(), Error> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::CoreErrors(errors))
    }
}

#[inline]
fn get_destination_path(
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
        let file_entry: FileEntry = TEST_DIR.into();
        let files: Result<std::vec::Vec<_>, _> = file_entry.into_iter().collect();
        let files = files?;
        assert_eq!(files.len(), TEST_DIR_FILES.len());
        assert_eq!(
            files,
            TEST_DIR_FILES
                .iter()
                .map(std::path::Path::new)
                .collect::<std::vec::Vec<_>>()
        );
        Ok(())
    }
    #[test]
    fn test_file_entry_fail() {
        let mut file_entry: FileEntry = WRONG_TEST_DIR.into();
        let next_file_entry = file_entry.next();
        assert!(next_file_entry.is_some());
        assert!(next_file_entry.unwrap().is_err());
        assert!(file_entry.next().is_none());
    }

    #[test]
    fn test_get_destination_path() -> Result<(), CoreError> {
        let destination_path = get_destination_path(
            std::path::Path::new("destination/root"),
            std::path::Path::new("source/root"),
            std::path::Path::new("source/root/some/file.txt"),
        )?;
        assert_eq!(
            destination_path,
            std::path::Path::new("destination/root/some/file.txt")
        );

        let destination_path = get_destination_path(
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
