#[derive(Clone, Debug)]
enum Error {
    SafeAll(safeall::Error),
    Tokio { join_error: String },
}

#[derive(Clone, Debug)]
enum Message {
    StartBackup,
    StartSync,
    StartRestore,
    StartSyncRestore,
    BackupUpdate(safeall::Message),
    BackupFinished(Result<(), Error>),
    ChooseSource,
    ChooseDestination,
    SourceFileChosen(Option<std::path::PathBuf>),
    DestinationFileChosen(Option<std::path::PathBuf>),
    NoSourceSet,
    NoDestinationSet,
    DestinationInputChanged(String),
    SourceInputChanged(String),
}

#[derive(Default, Debug)]
enum BackupState {
    #[default]
    Idle,
    Running {
        _task: iced::task::Handle,
    },
    Success,
    Error,
}

#[derive(Default, Debug)]
struct Gui {
    backup_state: BackupState,
    source: Option<std::path::PathBuf>,
    destination: Option<std::path::PathBuf>,
}

// TODO: https://github.com/harmony-development/Loqui/blob/master/src/screen/mod.rs#L1336
// https://docs.rs/directories/6.0.0/directories/
impl Gui {
    fn view(&self) -> iced::Element<'_, Message> {
        use iced::Length::Fill;
        use iced::widget::{
            button, center, center_x, column, container, progress_bar, row, scrollable, text,
            text_input, tooltip,
        };

        let progress_bar = progress_bar(0.0..=100.0, 0.0);
        let progress_info = "Nothing to do...";
        let hover_text = |t| {
            container(text(t).size(12))
                .padding(10)
                .style(container::rounded_box)
                .max_width(300)
        };
        let errors_and_warnings_text = "";
        let errors_and_warnings = column![
            text("Errors and Warnings:"),
            container(scrollable(text(errors_and_warnings_text)))
                .width(Fill)
                .height(Fill)
                .padding(10)
                .style(container::rounded_box)
        ];

        let content = column![
            column![
                row![
                    text_input(
                        "Choose source folder for your backup...",
                        self.source
                            .as_ref()
                            .map_or(String::new(), |p| p.display().to_string())
                            .as_str()
                    )
                    .on_input(Message::SourceInputChanged),
                    button("Choose").on_press(Message::ChooseSource),
                ]
                .spacing(5),
                row![
                    text_input(
                        "Choose destination folder for your backup...",
                        self.destination
                            .as_ref()
                            .map_or(String::new(), |p| p.display().to_string())
                            .as_str()
                    )
                    .on_input(Message::DestinationInputChanged),
                    button("Choose").on_press(Message::ChooseDestination),
                ]
                .spacing(5),
                column![text(progress_info).size(12), center_x(progress_bar),],
                center_x(
                    row![
                        tooltip(
                            button("Backup").on_press(Message::StartBackup),
                            hover_text("Will backup your source folder into your destination folder. This NEVER deletes a file which is in the destination but not in the source folder."),
                            tooltip::Position::Bottom
                        ),
                        tooltip(
                            button("Sync").on_press(Message::StartSync),
                            hover_text("Will make your destination identical with your source. This will DELETE files that are in the destination but not in the source folder."),
                            tooltip::Position::Bottom
                        ),
                        tooltip(
                            button("Restore").on_press(Message::StartRestore),
                            hover_text("Will restore your source folder from your destination folder. This will NEVER delete files that are in the source but not in the destination folder."),
                            tooltip::Position::Bottom
                        ),
                        tooltip(
                            button("Sync Restore").on_press(Message::StartSyncRestore),
                            hover_text("Will make your source folder identical with your destination folder. This will DELETE files that are in the source but not in the destination folder."),
                            tooltip::Position::Bottom
                        ),
                    ]
                    .spacing(20),
                ) 
            ].spacing(10),
            errors_and_warnings,
        ].spacing(20);

        center_x(content).padding([20, 20]).into()
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::StartBackup => {
                let Some(source_root) = self.source.clone() else {
                    return iced::Task::done(Message::NoSourceSet);
                };
                let Some(destination_root) = self.destination.clone() else {
                    return iced::Task::done(Message::NoDestinationSet);
                };
                self.start_backup(safeall::Command::Backup {
                    source_root,
                    destination_root,
                })
            }
            Message::StartSync => {
                println!("start sync");
                iced::Task::none()
            }
            Message::StartRestore => {
                println!("start restore");
                iced::Task::none()
            }
            Message::StartSyncRestore => {
                println!("start sync restore");
                iced::Task::none()
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
                self.source = path;
                iced::Task::none()
            }
            Message::ChooseDestination => {
                iced::Task::perform(Gui::choose_directory(), Message::DestinationFileChosen)
            }
            Message::DestinationFileChosen(path) => {
                self.destination = path;
                iced::Task::none()
            }
            Message::NoSourceSet => {
                println!("No source set.");
                iced::Task::none()
            }
            Message::NoDestinationSet => {
                println!("No destination set.");
                iced::Task::none()
            }
            Message::DestinationInputChanged(string) => {
                if string.is_empty() {
                    self.destination = None;
                } else {
                    use std::str::FromStr;
                    match std::path::PathBuf::from_str(&string) {
                        Ok(path) => self.destination = Some(path),
                        Err(error) => println!("{error}"),
                    }
                }

                iced::Task::none()
            }
            Message::SourceInputChanged(string) => {
                if string.is_empty() {
                    self.source = None;
                } else {
                    use std::str::FromStr;
                    match std::path::PathBuf::from_str(&string) {
                        Ok(path) => self.source = Some(path),
                        Err(error) => println!("{error}"),
                    }
                }

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

    fn theme(_: &Self) -> iced::Theme {
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
                    Ok(result) => result.map_err(Error::SafeAll),
                    Err(error) => Err(Error::Tokio {
                        join_error: error.to_string(),
                    }),
                }
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
