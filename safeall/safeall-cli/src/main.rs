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

async fn cli() -> Result<(), safeall::Error> {
    let cli_args = CliArgs::parse();
    safeall::run(cli_args.command.into()).await
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::process::ExitCode {
    if let Err(e) = cli().await {
        println!("ERROR: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
