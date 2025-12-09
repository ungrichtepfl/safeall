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

#[derive(Debug)]
enum CoreError {
    SourcePathDoesNotExist(std::path::PathBuf),
    CannotCreateDestinationDir(std::path::PathBuf, std::io::Error),
    DestinationIsNotADirectory(std::path::PathBuf),
    CannotReadDirectory(std::path::PathBuf, std::io::Error),
    CannotGetDirEntry(std::path::PathBuf, std::io::Error),
    CannotCreateNewDestinationDir(std::path::PathBuf, std::path::PathBuf),
    DestinationForSourceDirExistsAsFile(std::path::PathBuf, std::path::PathBuf),
    CannotCopyFile(std::path::PathBuf, std::path::PathBuf, std::io::Error),
}

impl std::error::Error for CoreError {}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::SourcePathDoesNotExist(path) => {
                write!(f, "Source path does not exist: {path:?}")
            }
            CoreError::CannotCreateDestinationDir(path, error) => {
                write!(f, "Cannot create destination path \"{path:?}\": {error}")
            }
            CoreError::DestinationIsNotADirectory(path) => {
                write!(f, "Destination is not a directory: {path:?}")
            }
            CoreError::CannotReadDirectory(path, error) => {
                write!(f, "Could not read source directory \"{path:?}\": {error}")
            }
            CoreError::CannotGetDirEntry(path, error) => {
                write!(f, "Could not read source directory \"{path:?}\": {error}")
            }
            CoreError::CannotCreateNewDestinationDir(source_path, current_destination) => {
                write!(
                    f,
                    "Could not create a new destination directory for path \"{source_path:?}\". Destination to append new basename: {current_destination:?}"
                )
            }
            CoreError::DestinationForSourceDirExistsAsFile(source_path, destination_dir) => {
                write!(
                    f,
                    "Could not create a new destination directory for path \"{source_path:?}\" because destination already exists but not as a directory: {destination_dir:?}"
                )
            }
            CoreError::CannotCopyFile(source_file, destination_file, error) => {
                write!(
                    f,
                    "Could not copy source \"{source_file:?}\" to destination file \"{destination_file:?}\": {error}"
                )
            }
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

    let dir_entry = match std::fs::read_dir(source)
        .map_err(|e| CoreError::CannotReadDirectory(source.into(), e))
    {
        Ok(dir_entry) => dir_entry,
        Err(e) => {
            errors.push(e);
            return errors;
        }
    };

    for entry in dir_entry {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if let Some(basename) = path.file_name() {
                    let new_destination = destination.join(basename);
                    if path.is_dir() {
                        if new_destination.exists() {
                            if !new_destination.is_dir() {
                                errors.push(CoreError::DestinationForSourceDirExistsAsFile(
                                    path,
                                    new_destination,
                                ));
                                continue;
                            }
                        } else if let Err(e) = std::fs::create_dir(&new_destination) {
                            errors.push(CoreError::CannotCreateDestinationDir(new_destination, e));
                            continue;
                        }
                        errors.append(&mut process_dir(&path, &new_destination));
                    } else {
                        // Is a file or a symbolic link
                        if let Err(e) = copy_if_newer(&path, &new_destination) {
                            errors.push(e);
                        }
                    }
                } else {
                    errors.push(CoreError::CannotCreateNewDestinationDir(
                        path,
                        destination.into(),
                    ));
                }
            }
            Err(e) => {
                errors.push(CoreError::CannotGetDirEntry(source.into(), e));
            }
        }
    }

    errors
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
    if destination_file.exists() {
        if let (Ok(src_metadata), Ok(dest_metadata)) =
            (source_file.metadata(), destination_file.metadata())
            && let (Ok(src_modification), Ok(dest_modification)) =
                (src_metadata.modified(), dest_metadata.modified())
        {
            if src_modification <= dest_modification
                && src_metadata.len() == dest_metadata.len()
                && src_metadata.file_type() == dest_metadata.file_type()
            {
                // FIXME: It is better to check if their content is still the same.
                println!(
                    "Skipping \"{source_file:?}\" as destination file \"{destination_file:?}\" is still up to date. "
                );
                return Ok(());
            }
        } else {
            // NOTE: If any of this fields cannot be read just log it, but try to copy the file anyway
            eprintln!(
                "WARINING: Cannot read all metadata of either {source_file:?} or {destination_file:?}."
            );
        }
    }

    // TODO: Implement check if it is newer.
    println!("Copy \"{source_file:?}\" to \"{destination_file:?}\"");
    if let Err(e) = std::fs::copy(source_file, destination_file) {
        Err(CoreError::CannotCopyFile(
            source_file.into(),
            destination_file.into(),
            e,
        ))
    } else {
        Ok(())
    }
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
