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

struct CliOutput {
    progress_bar: Option<indicatif::ProgressBar>,
    verbosity: Verbosity,
    warnings: Vec<safeall::Warning>,
}

enum Verbosity {
    Normal,
    Verbose,
}

impl Default for Verbosity {
    fn default() -> Self {
        Verbosity::Normal
    }
}

impl CliOutput {
    fn new(verbosity: Verbosity) -> Self {
        let progress_bar = None;
        Self {
            progress_bar,
            warnings: vec![],
            verbosity,
        }
    }
    fn process_message(&mut self, message: safeall::Message) {
        use safeall::Message as M;
        use safeall::Progress as P;
        match self.verbosity {
            Verbosity::Normal => match message {
                M::Warning(warning) => {
                    if let Some(ref progress_bar) = self.progress_bar {
                        progress_bar.set_message(format!("{warning}"));
                    }
                    self.warnings.push(warning)
                }
                M::Info(info) => {
                    if let Some(ref progress_bar) = self.progress_bar {
                        progress_bar.set_message(format!("{info}"));
                    }
                }
                M::Error(_) => {
                    // Do not display errors before the end
                }
                M::Progress(ref progress) => match progress {
                    P::Start(total, _) => {
                        self.create_progress_bar(*total, format!("{}", progress));
                    }
                    P::Increment(_) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar.inc(1);
                            progress_bar.set_message(format!("{}", progress));
                        }
                    }
                    P::End(_) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar.abandon_with_message(format!("{}", progress));
                        }
                        self.progress_bar = None;
                    }
                },
            },
            Verbosity::Verbose => match message {
                M::Warning(warning) => {
                    eprintln!("{}", console::style(format!("WARNING: {warning}")).yellow());
                }
                M::Info(info) => {
                    println!("{}", console::style(format!("INFO: {info}")).green());
                }
                M::Error(error) => {
                    eprintln!("{}", console::style(format!("ERROR: {error}")).red());
                }
                M::Progress(_) => {}
            },
        }
    }

    fn create_progress_bar(&mut self, length: usize, message: String) {
        let progress_bar = indicatif::ProgressBar::new(length as u64);
        progress_bar.set_style(
            indicatif::ProgressStyle::with_template("[{pos:>7}/{len:7}] {spinner} {wide_msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        progress_bar.set_message(message);
        self.progress_bar = Some(progress_bar);
    }
}

async fn cli() -> Result<(), Error> {
    let cli_args = CliArgs::parse();
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();
    let message_sender = CliMessageSender(message_sender);
    let run =
        tokio::spawn(async move { safeall::run(cli_args.command.into(), message_sender).await });

    let mut cli_output = CliOutput::new(Verbosity::default());

    while let Some(message) = message_receiver.recv().await {
        cli_output.process_message(message);
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
