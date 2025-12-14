#[derive(Clone)]
enum Message {
    StartBackup,
    StartSync,
    StartRestore,
    Finished(bool),
}

#[derive(Default)]
struct Gui {}

impl Gui {
    fn view(&self) -> iced::widget::Column<'_, Message> {
        use iced::widget::{button, column};
        column![
            button("Backup").on_press(Message::StartBackup),
            button("Sync").on_press(Message::StartSync),
            button("Restore").on_press(Message::StartRestore),
        ]
    }
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::StartBackup => iced::Task::perform(Gui::start_backup(), Message::Finished),
            Message::Finished(success) => {
                if !success {
                    eprintln!("Fail");
                } else {
                    println!("Success");
                }
                iced::Task::none()
            }
            Message::StartSync => {
                println!("Sync");
                iced::Task::none()
            }
            Message::StartRestore => {
                println!("Sync");
                iced::Task::none()
            }
        }
    }
    fn theme(&self) -> iced::Theme {
        iced::Theme::Light
    }
    async fn start_backup() -> bool {
        std::thread::sleep(std::time::Duration::from_secs(10));
        false
    }
}

fn main() -> Result<(), iced::Error> {
    iced::application(Gui::default, Gui::update, Gui::view)
        .theme(Gui::theme)
        .run()
}
