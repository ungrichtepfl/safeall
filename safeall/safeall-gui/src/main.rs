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
    OpenPath(std::path::PathBuf),
    OpenWindow,
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
    fn view(&self, window: iced::window::Id) -> iced::Element<'_, Message> {
        use iced::widget::{center_x, column};

        let content = column![
            self.view_title(),
            self.view_user_input(),
            self.view_progress(),
            self.view_errors_and_warnings(),
        ]
        .spacing(20);

        center_x(content).padding([20, 20]).into()
    }

    fn view_title(&self) -> iced::Element<'_, Message> {
        use iced::widget::{center_x, text};
        let title = center_x(text("Safeall").font(FONT_BOLD).size(28));

        title.into()
    }

    fn view_user_input(&self) -> iced::Element<'_, Message> {
        use iced::border;
        use iced::widget::{button, center_x, column, container, row, text, text_input, tooltip};

        let directory_input = move |txt, on_input: fn(String) -> Message, on_press| {
            row![
                text_input(
                    txt,
                    self.source
                        .as_ref()
                        .map_or(String::new(), |p| p.display().to_string())
                        .as_str()
                )
                .on_input(on_input),
                button("Select")
                    .on_press(on_press)
                    .height(30)
                    .style(|theme, status| {
                        let mut style = button::primary(theme, status);
                        style.border.radius = border::Radius::new(15);
                        style
                    })
            ]
            .spacing(5)
        };

        let choose_directories = column![
            directory_input(
                "Choose source folder for your backup...",
                Message::SourceInputChanged,
                Message::ChooseSource
            ),
            directory_input(
                "Choose destination folder for your backup...",
                Message::DestinationInputChanged,
                Message::ChooseDestination
            ),
        ]
        .spacing(5);

        let hover_text = move |txt| {
            container(text(txt).size(12))
                .padding(10)
                .style(container::bordered_box)
                .max_width(300)
        };

        let backup_button = move |button_txt, on_press, hover_txt| {
            tooltip(
                button(button_txt)
                    .on_press(on_press)
                    .height(30)
                    .style(|theme, status| {
                        let mut style = button::primary(theme, status);
                        style.border.radius = border::Radius::new(15);
                        style
                    }),
                hover_text(hover_txt),
                tooltip::Position::Bottom,
            )
        };

        let backup_buttons = center_x(
            row![
                backup_button(
                    "Backup",
                    Message::StartBackup,
                    "Will backup your source folder into your destination folder. \
                        This NEVER deletes a file which is in the destination but not in \
                        the source folder."
                ),
                backup_button(
                    "Sync",
                    Message::StartSync,
                    "Will make your destination identical with your source. \
                        This will DELETE files that are in the destination but not in \
                        the source folder."
                ),
                backup_button(
                    "Restore",
                    Message::StartRestore,
                    "Will restore your source folder from your destination folder. \
                        This will NEVER delete files that are in the source but not in \
                        the destination folder."
                ),
                backup_button(
                    "Sync Restore",
                    Message::StartSyncRestore,
                    "Will make your source folder identical with your destination folder.\
                        This will DELETE files that are in the source but not in \
                        the destination folder."
                ),
            ]
            .spacing(20),
        );

        let user_input = column![choose_directories, backup_buttons].spacing(10);
        user_input.into()
    }

    fn view_progress(&self) -> iced::Element<'_, Message> {
        use iced::widget::{center_x, column, progress_bar, text};

        let progress_bar = progress_bar(0.0..=100.0, 32.0);
        let progress_info = "Nothing to do...";
        let progress = column![text(progress_info).size(12), center_x(progress_bar)];
        progress.into()
    }

    fn view_errors_and_warnings(&self) -> iced::Element<'_, Message> {
        const ROWS: [&str; 20] = ["/home/chrigi"; 20];

        use iced::Length::Fill;
        use iced::alignment::Alignment::Center;
        use iced::alignment::Horizontal::Right;
        use iced::widget::{button, column, container, scrollable, table, text};

        let columns = {
            let bold = |header| text(header).font(FONT_BOLD);
            [
                table::column(bold("File"), |path: &std::path::Path| {
                    button(text(path.display().to_string()))
                        .style(button::text)
                        .on_press(Message::OpenPath(path.to_owned()))
                })
                .align_y(Center),
                table::column(bold("Error"), |path: &std::path::Path| text("Some Error"))
                    .align_y(Center)
                    .width(Fill),
            ]
        };

        let errors_and_warnings = column![
            text("Errors and Warnings:"),
            container(scrollable(table(columns, ROWS.map(std::path::Path::new))))
                .width(Fill)
                .height(Fill)
                .padding(10)
                .style(container::rounded_box)
        ];
        errors_and_warnings.into()
    }

    fn boot() -> (Self, iced::Task<Message>) {
        (Self::default(), iced::Task::done(Message::OpenWindow))
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
            Message::OpenPath(path) => {
                // TODO: Implement https://stackoverflow.com/questions/66485945/with-rust-open-explorer-on-a-file
                std::process::Command::new("xdg-open")
                    .arg(path.to_string_lossy().to_string()) // <- Specify the directory you'd like to open.
                    .spawn()
                    .unwrap();
                iced::Task::none()
            }
            Message::OpenWindow => {
                let settings = iced::window::Settings::default();
                let (_, task) = iced::window::open(settings);
                task.then(
                    // TODO: Maybe send message that window has been opened?
                    |_| iced::Task::none(),
                )
            }
        }
    }

    async fn choose_directory() -> Option<std::path::PathBuf> {
        rfd::AsyncFileDialog::new()
            .pick_folder()
            .await
            .map(|d| d.path().to_owned())
    }

    fn theme(&self, window: iced::window::Id) -> iced::Theme {
        iced::Theme::CatppuccinLatte
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
const FONT_BYTES: &[u8] = include_bytes!("../fonts/Roboto.ttf");
const FONT_REGULAR: iced::Font = iced::Font::with_name("Roboto");
const FONT_BOLD: iced::Font = iced::Font {
    weight: iced::font::Weight::Bold,
    ..FONT_REGULAR
};

fn get_tray_icon_attributes() -> tray_icon::TrayIconAttributes {
    let menu = tray_icon::menu::Menu::new();
    let show_item = tray_icon::menu::MenuItem::new("Show", true, None);
    let quit_item = tray_icon::menu::MenuItem::new("Quit", true, None);
    if cfg!(target_os = "macos") {
        // NOTE: Appending a menu item directly does not work on macos
        let submenu = tray_icon::menu::Submenu::new("Settings", true);
        submenu.append(&show_item).unwrap();
        submenu.append(&quit_item).unwrap();
        menu.append(&submenu).unwrap();
    } else {
        menu.append(&show_item).unwrap();
        menu.append(&quit_item).unwrap();
    }

    let icon = tray_icon::Icon::from_rgba([50; 16 * 16 * 4].to_vec(), 16, 16).unwrap();
    tray_icon::TrayIconAttributes {
        icon: Some(icon),
        menu: Some(Box::new(menu)),
        tooltip: Some("Safeall".to_string()),
        ..Default::default()
    }
}

fn main() -> Result<(), iced::Error> {
    // NOTE:
    //  https://github.com/ssrlive/iced-demo/blob/master/src/main.rs
    //  https://github.com/tauri-apps/tray-icon/issues/252
    // FIXME: On macOS it must be spawn in the main loop
    std::thread::spawn(move || {
        let attrs = get_tray_icon_attributes();

        #[cfg(target_os = "linux")]
        gtk::init().unwrap();

        let _tray_icon = tray_icon::TrayIcon::new(attrs).unwrap();
        loop {
            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                println!("{event:?}");
            }

            #[cfg(target_os = "linux")]
            gtk::main_iteration();
        }
    });

    iced::daemon(Gui::boot, Gui::update, Gui::view)
        .theme(Gui::theme)
        .font(FONT_BYTES)
        .default_font(FONT_REGULAR)
        .run()
}
