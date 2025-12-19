use clap::Parser;

const STYLES: clap::builder::styling::Styles = clap::builder::styling::Styles::styled()
    .header(clap::builder::styling::AnsiColor::Green.on_default().bold())
    .usage(clap::builder::styling::AnsiColor::Green.on_default().bold())
    .literal(clap::builder::styling::AnsiColor::Blue.on_default().bold())
    .placeholder(clap::builder::styling::AnsiColor::Cyan.on_default());

/// Backup or sync your filesystem to/from another folder.
#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(styles=STYLES)]
struct CliArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Backup files from source directory to destination directory.
    /// It does not delete files in the destination directory.
    Backup {
        /// Folder which you want to backup
        source_root: String,
        /// Folder which will be your backup
        destination_root: String,
    },
    /// Sync destination directory from source directory.
    /// This delets files in the destination directory when they do not exist in the source directory.
    Sync {
        /// Folder which you want to backup
        source_root: String,
        /// Folder which will be your backup
        destination_root: String,
    },
    /// Restore the source directory from the destination directory.
    Restore {
        /// Folder which you want to restore from the backup
        source_root: String,
        /// Folder where you have your backup
        destination_root: String,
        /// If you want to delete the files that are not in your backup
        #[arg(short, long)]
        delete_files: bool,
    },
}

impl From<Commands> for safeall::Command {
    fn from(commands: Commands) -> Self {
        match commands {
            Commands::Backup {
                source_root,
                destination_root,
            } => safeall::Command::Backup {
                source_root: source_root.into(),
                destination_root: destination_root.into(),
            },
            Commands::Sync {
                source_root,
                destination_root,
            } => safeall::Command::Sync {
                source_root: source_root.into(),
                destination_root: destination_root.into(),
            },
            Commands::Restore {
                source_root,
                destination_root,
                delete_files,
            } => safeall::Command::Restore {
                source_root: source_root.into(),
                destination_root: destination_root.into(),
                delete_files,
            },
        }
    }
}

#[derive(Debug)]
enum Error {
    Core(safeall::Error),
    Tokio(tokio::task::JoinError),
}

impl std::error::Error for Error {}

impl From<tokio::task::JoinError> for Error {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::Tokio(value)
    }
}
impl From<safeall::Error> for Error {
    fn from(value: safeall::Error) -> Self {
        Self::Core(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(e) => e.fmt(f),
            Self::Tokio(e) => e.fmt(f),
        }
    }
}

struct CliMessageSender(tokio::sync::mpsc::UnboundedSender<safeall::Message>);

impl safeall::MessageSender for CliMessageSender {
    fn send(&self, message: safeall::Message) {
        self.0.send(message).ok();
    }
}

async fn cli() -> Result<(), Error> {
    let cli_args = CliArgs::parse();
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();
    let message_sender = CliMessageSender(message_sender);
    let run =
        tokio::spawn(async move { safeall::run(cli_args.command.into(), message_sender).await });

    while let Some(message) = message_receiver.recv().await {
        use safeall::Message as M;
        match message {
            M::Warning(warning) => match warning {
                safeall::Warning::CannotGetMetadata {
                    source,
                    destination,
                    copy_anyway,
                } => {
                    let copy_text = if copy_anyway {
                        "We try to copy the file anyway."
                    } else {
                        "File will not be backed up."
                    };
                    println!(
                        "WARNING: Cannot get metadata for either \"{}\" or \"{}\". {copy_text}",
                        source.display(),
                        destination.display()
                    )
                }
                safeall::Warning::CannotGetHash {
                    source,
                    destination,
                    copy_anyway,
                } => {
                    let copy_text = if copy_anyway {
                        "We try to copy the file anyway."
                    } else {
                        "File will not be backed up."
                    };
                    println!(
                        "WARNING: Cannot get the hashes for either \"{}\" or \"{}\". {copy_text}",
                        source.display(),
                        destination.display()
                    )
                }
                safeall::Warning::CannotCopyModifiedTime {
                    source,
                    destination,
                } => println!(
                    "WARNING: Cannot copy the modification times from \"{}\" to \"{}\".",
                    source.display(),
                    destination.display()
                ),
            },
            M::Info(info) => match info {
                safeall::Info::SkippingFileNoModification {
                    source,
                    destination,
                } => println!(
                    "Skipping \"{}\", \"{}\" is up to date.",
                    source.display(),
                    destination.display()
                ),
                safeall::Info::StartCopingFile {
                    source,
                    destination,
                } => println!(
                    "Start backing up \"{}\" to \"{}\".",
                    source.display(),
                    destination.display()
                ),
                safeall::Info::DestinationDirCreated(path_buf) => {
                    println!("Creating directory \"{}\".", path_buf.display(),)
                }
                safeall::Info::DirCreated {
                    source,
                    destination,
                } => println!(
                    "Created \"{}\" for \"{}\".",
                    destination.display(),
                    source.display(),
                ),
                safeall::Info::DestinationDirAlreadyExists {
                    source,
                    destination,
                } => println!(
                    "\"{}\" for \"{}\" already exists, will not be created.",
                    destination.display(),
                    source.display(),
                ),
                safeall::Info::CreatingDestinationDir(path_buf) => {
                    println!("Creating backup destination \"{}\".", path_buf.display(),)
                }
                safeall::Info::FileCopied {
                    source,
                    destination,
                } => println!(
                    "Finished backing up \"{}\" to \"{}\".",
                    source.display(),
                    destination.display()
                ),
                safeall::Info::StartDeletingDir(path_buf) => {
                    println!("Start deleting directory \"{}\".", path_buf.display(),)
                }
                safeall::Info::StartDeletingFile(path_buf) => {
                    println!("Start deleting file \"{}\".", path_buf.display(),)
                }
                safeall::Info::DeletedFile(path_buf) => {
                    println!("Finished deleting file \"{}\".", path_buf.display(),)
                }
                safeall::Info::DeletedDir(path_buf) => {
                    println!("Finished deleting directory \"{}\".", path_buf.display(),)
                }
            },
            M::Error(error) => eprintln!("ERROR: {error}"),
            M::Progress {
                progress: _,
                done: _,
                total: _,
            } => {}
        }
    }

    let res = run.await?;
    res?;
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::process::ExitCode {
    if let Err(e) = cli().await {
        println!("ERROR: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
