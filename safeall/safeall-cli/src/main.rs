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

async fn cli() -> Result<(), Error> {
    let cli_args = CliArgs::parse();
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();
    let run =
        tokio::spawn(async move { safeall::run(cli_args.command.into(), message_sender).await });

    let bar = indicatif::ProgressBar::no_length();
    let style = indicatif::ProgressStyle::default_bar()
        .template("{wide_msg}\n{pos:>7}/{len:7} {bar:40.cyan/blue}")
        .unwrap();
    bar.set_style(style);
    let mut total = 0;
    let mut done = 0;
    while let Some(message) = message_receiver.recv().await {
        use safeall::Message as M;
        match message {
            M::Warning(warning) => println!("WARNING: {warning}"),
            M::Info(info) => bar.set_message(info),
            M::Progress {
                progress: _,
                done: d,
                total: t,
            } => {
                bar.set_length(t as u64);
                total = t;
                bar.set_position(d as u64);
                done = d;
            }
        }
    }
    bar.finish_and_clear();
    println!("Done!");
    println!("{done}/{total} files have been backed up.");

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
