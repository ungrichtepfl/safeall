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
    #[arg(short, long)]
    verbose: bool,
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

struct CliMessageSender(tokio::sync::mpsc::UnboundedSender<safeall::Message>);

impl safeall::MessageSender for CliMessageSender {
    fn send(&self, message: safeall::Message) {
        self.0.send(message).ok();
    }
}

struct CliOutput {
    progress_bar: Option<indicatif::ProgressBar>,
    verbosity: Verbosity,
}

enum Verbosity {
    Normal,
    Verbose,
}

mod style {
    pub fn progress_bar_style(dottet_style: &str) -> indicatif::ProgressStyle {
        indicatif::ProgressStyle::with_template(&format!(
            "[{{pos:>7}}/{{len:7}}] {{wide_msg:.{dottet_style}}}"
        ))
        .unwrap()
    }

    pub fn warning() -> console::Style {
        console::Style::from_dotted_str(warning_dotted())
    }
    pub fn success() -> console::Style {
        console::Style::from_dotted_str(success_dotted())
    }
    pub fn info() -> console::Style {
        console::Style::from_dotted_str(info_dotted())
    }
    pub fn increment_fail() -> console::Style {
        console::Style::from_dotted_str(increment_fail_dotted())
    }
    pub fn error() -> console::Style {
        console::Style::from_dotted_str(error_dotted())
    }
    pub fn increment_success() -> console::Style {
        console::Style::from_dotted_str(increment_success_dotted())
    }

    pub const fn warning_dotted() -> &'static str {
        "yellow"
    }
    pub const fn success_dotted() -> &'static str {
        "green"
    }
    pub const fn info_dotted() -> &'static str {
        ""
    }
    pub const fn increment_fail_dotted() -> &'static str {
        "red"
    }
    pub const fn error_dotted() -> &'static str {
        "124"
    }
    pub const fn increment_success_dotted() -> &'static str {
        "7"
    }
}

impl CliOutput {
    fn new(verbosity: Verbosity) -> Self {
        let progress_bar = None;
        Self {
            progress_bar,
            verbosity,
        }
    }
    fn process_message(&mut self, message: safeall::Message) {
        use safeall::Message as M;
        use safeall::Progress as P;
        match self.verbosity {
            Verbosity::Normal => match message {
                M::Warning(warning) => {
                    // if let Some(ref progress_bar) = self.progress_bar {
                    //     progress_bar.set_style(Self::warning_style());
                    //     progress_bar.set_message(format!("{warning}"));
                    // }
                    // self.warnings.push(warning);
                }
                M::Info(info) => {
                    // if let Some(ref progress_bar) = self.progress_bar {
                    //     progress_bar.set_style(Self::info_style());
                    //     progress_bar.set_message(format!("{info}"));
                    // }
                }
                M::Progress(ref progress) => match progress {
                    P::Start(total, _) => {
                        self.create_progress_bar(*total, format!("{progress}"));
                    }
                    P::IncrementSuccess(_) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar.set_style(style::progress_bar_style(
                                style::increment_success_dotted(),
                            ));
                            progress_bar.set_message(format!("{progress}"));
                            progress_bar.inc(1);
                        }
                    }
                    P::EndFail(_, _) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar
                                .set_style(style::progress_bar_style(style::increment_fail_dotted()));
                            progress_bar.abandon_with_message(format!("{progress}"));
                        }
                        self.progress_bar = None;
                    }
                    P::EndSuccess(_) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar
                                .set_style(style::progress_bar_style(style::success_dotted()));
                            progress_bar.abandon_with_message(format!("{progress}"));
                        }
                        self.progress_bar = None;
                    }
                    P::IncrementFail(_) => {
                        if let Some(ref progress_bar) = self.progress_bar {
                            progress_bar.set_style(style::progress_bar_style(
                                style::increment_fail_dotted(),
                            ));
                            progress_bar.set_message(format!("{progress}"));
                            // progress_bar.tick();
                        }
                    }
                },
            },
            Verbosity::Verbose => match message {
                M::Warning(warning) => {
                    eprintln!(
                        "{}",
                        style::warning().apply_to(format!("WARNING: {warning}"))
                    );
                }
                M::Info(info) => {
                    println!("{}", style::info().apply_to(format!("INFO: {info}")));
                }
                M::Progress(progress) => {
                    let style = match progress {
                        P::IncrementSuccess(_) => {
                            style::increment_success().apply_to(format!("INFO: {progress}"))
                        }
                        P::IncrementFail(_) => {
                            style::increment_fail().apply_to(format!("ERROR: {progress}"))
                        }
                        P::EndSuccess(_) => style::success().apply_to(format!("INFO: {progress}")),
                        P::EndFail(_, _) => style::error().apply_to(format!("ERROR: {progress}")),
                        P::Start(_, _) => style::info().apply_to(format!("INFO: {progress}")),
                    };
                    println!("{style}");
                }
            },
        }
    }

    fn create_progress_bar(&mut self, length: usize, message: String) {
        let progress_bar = indicatif::ProgressBar::new(length as u64);
        progress_bar.set_style(style::progress_bar_style(style::info_dotted()));
        progress_bar.set_message(message);
        self.progress_bar = Some(progress_bar);
    }
}

async fn cli() -> bool {
    let cli_args = CliArgs::parse();
    let verbosity = if cli_args.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();
    let message_sender = CliMessageSender(message_sender);
    let run =
        tokio::spawn(async move { safeall::run(cli_args.command.into(), message_sender).await });

    let mut cli_output = CliOutput::new(verbosity);

    while let Some(message) = message_receiver.recv().await {
        cli_output.process_message(message);
    }

    match run.await {
        Ok(result) => {
            if let Err(error) = result {
                eprintln!("{}", style::error().apply_to(format!("ERROR: {error}")));
                return false;
            }
        }
        Err(error) => {
            eprintln!(
                "{}",
                style::error().apply_to(format!("ERROR: Could execute command: {error}"))
            );
            return false;
        }
    }
    true
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::process::ExitCode {
    if !cli().await {
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
