#[derive(Clone)]
enum Message {
    StartBackup,
    StartSync,
    StartRestore,
    BackupUpdate(safeall::Message),
    BackupFinished(Result<(), safeall::Error>),
    ChooseSource,
    SourceFileChosen(Option<std::path::PathBuf>),
}

#[derive(Default)]
enum BackupState {
    #[default]
    Idle,
    Running {
        _task: iced::task::Handle,
    },
    Success,
    Error,
}

#[derive(Default)]
struct Gui {
    backup_state: BackupState,
}
// TODO: https://github.com/harmony-development/Loqui/blob/master/src/screen/mod.rs#L1336
// https://docs.rs/directories/6.0.0/directories/
impl Gui {
    fn view(&self) -> iced::widget::Column<'_, Message> {
        use iced::widget::{button, column};
        column![
            button("Backup").on_press(Message::StartBackup),
            button("Sync").on_press(Message::StartSync),
            button("Restore").on_press(Message::StartRestore),
            button("Choose").on_press(Message::ChooseSource),
        ]
    }
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::StartBackup => self.start_backup(safeall::Command::Backup {
                source_root: std::path::PathBuf::from("out"),
                destination_root: std::path::PathBuf::from("target"),
            }),
            Message::StartSync => {
                todo!();
            }
            Message::StartRestore => {
                todo!();
            }
            Message::BackupUpdate(message) => {
                println!("{message:?}");
                iced::Task::none()
            }
            Message::BackupFinished(result) => {
                println!("{result:?}");
                iced::Task::none()
            }
            Message::ChooseSource => {
                iced::Task::perform(Gui::choose_directory(), Message::SourceFileChosen)
            }
            Message::SourceFileChosen(path) => {
                println!("{path:?}");
                iced::Task::none()
            }
        }
    }

    async fn choose_directory() -> Option<std::path::PathBuf> {
        rfd::AsyncFileDialog::new()
            .pick_folder()
            .await
            .map(|d| d.path().to_owned())
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Light
    }

    fn start_backup(&mut self, command: safeall::Command) -> iced::Task<Message> {
        let (task, handle) = iced::Task::sip(
            iced::task::sipper(async move |mut iced_sender| {
                let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();
                let run = tokio::spawn(async move { safeall::run(command, message_sender).await });

                while let Some(message) = message_receiver.recv().await {
                    iced_sender.send(message).await;
                }

                match run.await {
                    Ok(result) => {
                        return result;
                    }
                    Err(error) => {}
                }
                Ok(())
            }),
            Message::BackupUpdate,
            Message::BackupFinished,
        )
        .abortable();

        self.backup_state = BackupState::Running {
            _task: handle.abort_on_drop(),
        };

        task
    }
}

fn main() -> Result<(), iced::Error> {
    iced::application(Gui::default, Gui::update, Gui::view)
        .theme(Gui::theme)
        .run()
}
