// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::thread;

use crate::{fl, player};
use std::fs;
use cosmic::app::{Command, Core};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, button, Column, Container, icon, menu, nav_bar, text};
use cosmic::{cosmic_theme, theme, Application, ApplicationExt, Apply, Element};
use log::error;

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink, source::Source};
use rodio::source::SineWave;

// use cosmic_files::{
//     dialog::{Dialog, DialogKind, DialogMessage, DialogResult},
//     mime_icon::{mime_for_path, mime_icon},
// };

use cosmic::dialog::file_chooser::{self, FileFilter};
use url::Url;

const REPOSITORY: &str = "https://github.com/edfloreshz/cosmic-app-template";

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
pub struct Yamp {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    /// A model that contains all of the pages assigned to the nav bar panel.
    nav: nav_bar::Model,
    /// A vector that contains the list of scanned files
    scanned_files: Vec<PathBuf>,
    thing: Arc<i32>,
    audio_player: RodioAudioPlayer
}

/// The AudioPlayer struct handles audio playback using the rodio backend.
pub struct RodioAudioPlayer {
    /// The sink responsible for managing the audio playback.
    player: rodio::Sink,
    /// Keep OutputStream alive. Otherwise audio playback will NOT work
    /// (sorry I don't make the rules)
    _stream: rodio::OutputStream,
    /// Store content for rewind/replay
    content: Vec<u8>,
}

/// This is the enum that contains all the possible variants that your application will need to transmit messages.
/// This is used to communicate between the different parts of your application.
/// If your application does not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    Scan,
    Play,
    Cancelled,
    CloseError,
    Error(String),
    FileRead(Url, String),
    OpenError(Arc<file_chooser::Error>),
    OpenFile,
    Selected(Url),
}

/// Identifies a page in the application.
pub enum Page {
    Page1,
    Page2,
    Page3,
}

/// Identifies a context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::About => fl!("about"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    Play,
    Scan,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Play => { Message::Play }
            MenuAction::Scan => { Message::Scan }
        }
    }
}

/// Implement the `Application` trait for your application.
/// This is where you define the behavior of your application.
///
/// The `Application` trait requires you to define the following types and constants:
/// - `Executor` is the async executor that will be used to run your application's commands.
/// - `Flags` is the data that your application needs to use before it starts.
/// - `Message` is the enum that contains all the possible variants that your application will need to transmit messages.
/// - `APP_ID` is the unique identifier of your application.
impl Application for Yamp {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "com.example.CosmicAppTemplate";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Instructs the cosmic runtime to use this model as the nav bar model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// This is the entry point of your application, it is where you initialize your application.
    ///
    /// Any work that needs to be done before the application starts should be done here.
    ///
    /// - `core` is used to passed on for you by libcosmic to use in the core of your own application.
    /// - `flags` is used to pass in any data that your application needs to use before it starts.
    /// - `Command` type is used to send messages to your application. `Command::none()` can be used to send no messages to your application.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text("Page 1")
            .data::<Page>(Page::Page1)
            .icon(icon::from_name("applications-science-symbolic"))
            .activate();

        nav.insert()
            .text("Page 2")
            .data::<Page>(Page::Page2)
            .icon(icon::from_name("applications-system-symbolic"));

        nav.insert()
            .text("Page 3")
            .data::<Page>(Page::Page3)
            .icon(icon::from_name("applications-games-symbolic"));

        let scanned_files = vec![];

        let thing = Arc::new(5);

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let player = Sink::try_new(&stream_handle).unwrap();
        let mut content = Vec::new();

        let mut audio_player = RodioAudioPlayer {
            player,
            _stream,
            content
        };

        let mut app = Yamp {
            core,
            context_page: ContextPage::default(),
            key_binds: HashMap::new(),
            nav,
            scanned_files,
            thing,
            audio_player
        };


        let command = app.update_titles();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(fl!("about"), MenuAction::About)],
            ),
        ),
            menu::Tree::with_children(
                menu::root(fl!("debug")),
                menu::items(
                    &self.key_binds,
                    vec![menu::Item::Button(fl!("debug-play"), MenuAction::Play),
                         menu::Item::Button(fl!("debug-file-listing"), MenuAction::Scan)],
                ),
            )

        ]);

        vec![menu_bar.into()]
    }

    /// This is the main view of your application, it is the root of your widget tree.
    ///
    /// The `Element` type is used to represent the visual elements of your application,
    /// it has a `Message` associated with it, which dictates what type of message it can send.
    ///
    /// To get a better sense of which widgets are available, check out the `widget` module.
    fn view(&self) -> Element<Self::Message> {
        // self.nav.text() - pass it a nav item from the model to get its text
        // self.nav.active() - get currently active nav
        let mut col = Column::new();

        // MOVED IN TO DEBUG MENU
        // //if let Some(page) = self.nav.text(self.nav.active()) {
        //     let txt = text(fl!("scan"));
        //     let txt_container = Container::new(txt).center_x().width(Length::Fill);
        //     let btn = button(txt_container).on_press(Message::Scan);
        //
        //     col = col.push(btn);
        // //} else {
        //     //todo I guess?
        // //}

        let txt_open = text(fl!("get-files"));
        let txt_open_container = Container::new(txt_open).center_x().width(Length::Fill);
        let btn_open = button(txt_open_container).on_press(Message::OpenFile);

        col = col.push(btn_open);

        // MOVED IN TO DEBUG MENU
        // let txt_play = text(fl!("play"));
        // let txt_play_container = Container::new(txt_play).center_x().width(Length::Fill);
        // let btn_play = button(txt_play_container).on_press(Message::Play);
        //
        // col = col.push(btn_play);

        // https://hermanradtke.com/2015/06/22/effectively-using-iterators-in-rust.html/
        for file in &self.scanned_files {
            println!("Name: {}", file.display());
            println!("hol up: {}", file.display());
            let file_txt = text(file.display().to_string());
            let file_txt_container = Container::new(file_txt).center_x().width(Length::Fill);

            col = col.push(file_txt_container);
        }

        let widg = widget::text::title1(fl!("welcome"))
            .apply(widget::container)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center);

        col = col.push(widg);

        col.into()
    }

    /// Application messages are handled here. The application state can be modified based on
    /// what message was received. Commands may be returned for asynchronous execution on a
    /// background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::LaunchUrl(url) => {
                let _result = open::that_detached(url);
            }

            Message::Scan => {
                let paths = fs::read_dir("./").unwrap();

                for path in paths {
                    //println!("Name: {}", path.unwrap().path().display());
                    self.scanned_files.push(path.unwrap().path());
                }

                self.audio_player.player.pause();

                // for file in &self.scanned_files {
                //     println!("Name: {}", file.display());
                // }
            }

            // Via cosmic-edit
            // Message::OpenFileDialog => {
            //     if self.dialog_opt.is_none() {
            //         let (dialog, command) = Dialog::new(
            //             DialogKind::OpenMultipleFiles,
            //             None,
            //             Message::DialogMessage,
            //             Message::OpenFileResult,
            //         );
            //         self.dialog_opt = Some(dialog);
            //         return command;
            //     }
            // }
            //
            // Message::OpenProjectDialog => {
            //     if self.dialog_opt.is_none() {
            //         let (dialog, command) = Dialog::new(
            //             DialogKind::OpenMultipleFolders,
            //             None,
            //             Message::DialogMessage,
            //             Message::OpenProjectResult,
            //         );
            //         self.dialog_opt = Some(dialog);
            //         return command;
            //     }
            // }

            // Creates a new open dialog.
            // https://github.com/pop-os/libcosmic/blob/master/examples/open-dialog/src/main.rs
            Message::OpenFile => {
                return cosmic::command::future(async move {
                    eprintln!("opening new dialog");

                    #[cfg(feature = "rfd")]
                    let filter = FileFilter::new("Music files").extension("mp3");

                    #[cfg(feature = "xdg-portal")]
                    let filter = FileFilter::new("Music files").glob("*.mp3");

                    let dialog = file_chooser::open::Dialog::new()
                        // Sets title of the dialog window.
                        .title("Choose a file")
                        // Accept only plain text files
                        .filter(filter);

                    match dialog.open_file().await {
                        Ok(response) => Message::Selected(response.url().to_owned()),

                        Err(file_chooser::Error::Cancelled) => Message::Cancelled,

                        Err(why) => Message::OpenError(Arc::new(why)),
                    }
                });
            }

            // Displays an error in the application's warning bar.
            Message::Error(why) => {
                //self.error_status = Some(why);
            }

            // Displays an error in the application's warning bar.
            Message::OpenError(why) => {
                // if let Some(why) = Arc::into_inner(why) {
                //     let mut source: &dyn std::error::Error = &why;
                //     let mut string =
                //         format!("open dialog subscription errored\n    cause: {source}");
                //
                //     while let Some(new_source) = source.source() {
                //         string.push_str(&format!("\n    cause: {new_source}"));
                //         source = new_source;
                //     }
                //
                //     self.error_status = Some(string);
                // }
            }

            Message::Cancelled => {}
            Message::CloseError => {}
            Message::FileRead(_, _) => {}

            Message::Selected(url) => {
                eprintln!("selected file");
                //
                // // Take existing file contents buffer to reuse its allocation.
                // let mut contents = String::new();
                // std::mem::swap(&mut contents, &mut self.file_contents);
                //
                // // Set the file's URL as the application title.
                // self.set_header_title(url.to_string());
                //
                // // Reads the selected file into memory.
                // return cosmic::command::future(async move {
                //     // Check if its a valid local file path.
                //     let path = match url.scheme() {
                //         "file" => url.to_file_path().unwrap(),
                //         other => {
                //             return Message::Error(format!("{url} has unknown scheme: {other}"));
                //         }
                //     };
                //
                //     // Open the file by its path.
                //     let mut file = match tokio::fs::File::open(&path).await {
                //         Ok(file) => file,
                //         Err(why) => {
                //             return Message::Error(format!(
                //                 "failed to open {}: {why}",
                //                 path.display()
                //             ));
                //         }
                //     };
                //
                //     // Read the file into our contents buffer.
                //     contents.clear();
                //
                //     if let Err(why) = file.read_to_string(&mut contents).await {
                //         return Message::Error(format!("failed to read {}: {why}", path.display()));
                //     }
                //
                //     contents.shrink_to_fit();
                //
                //     // Send this back to the application.
                //     Message::FileRead(url, contents)
                // });

                println!("{}", url.as_str());
                println!("{:?}", url.to_file_path().unwrap());
                println!("{}", Path::new(url.as_str()).is_file());

                let file = BufReader::new(File::open(url.to_file_path().unwrap()).unwrap());

                let source = Decoder::new(file).unwrap();

                // TURNS OUT I JUST HAD WRAP THIS SUCKER IN A STRUCT
                // I THINK BECAUSE THE STREAM NEEDS TO STAY ALIVE OR AUDIO WON'T PLAY
                // (_STREAM IS A FIELD IN THIS STRUCT)
                // (tested without the _stream field and audio didn't work jsyk)
                // SLEEP UNTIL END IS COMPLETELY UNNECESSARY IN THIS CASE BECAUSE
                // LIBCOSMIC KEEPS THE MAIN THREAD ALIVE
                // AND I DON'T HAVE TO MAKE A SECOND THREAD BECAUSE RODIO IS ALREADY DOING THAT
                // IN THE BACKGROUND
                // HOLY SMOKES
                self.audio_player.player.append(source);
                // WHEN YOU APPEND A SOURCE TO THE PLAYER IT IMMEDIATELY STARTS PLAYING
                // BUT IF YOU PAUSE AND APPEND ANOTHER THING IT DON'T START PLAYING AGAIN
                // THEREFORE, WE MAKE SURE TO CALL PLAY EVERY TIME.
                self.audio_player.player.play();
            }


            Message::Play => {

               // let sinker = Arc::clone(&sink_ultimate);
               //
               //  let thing_ult = Arc::new(5);
               //  let thing = Arc::clone(&thing_ult);

                // _stream must live as long as the sink
                // let (_stream, stream_handle) = OutputStream::try_default().unwrap();
                // let sink_ultimate = Arc::new(Sink::try_new(&stream_handle).unwrap());
                // let sinker_ultimate = Arc::clone(&sink_ultimate);

                // let (_stream, stream_handle) = OutputStream::try_default().unwrap();
                // let sinker_ultimate = Sink::try_new(&stream_handle).unwrap();

                // for some reason moving ownership of the sink into another thread (using Arc
                // or not) the sound doesn't play and anything after sleep_until_end doesn't trigger.

                // https://doc.rust-lang.org/book/ch13-01-closures.html
                // let handle = thread::spawn(move || {
                //     // Load a sound from a file, using a path relative to Cargo.toml
                //     let file = BufReader::new(File::open("/home/ben/Projects/YAMP/res/sample.flac").unwrap());
                //     // Decode that sound file into a source
                //     let source = Decoder::new(file).unwrap();
                //     sinker_ultimate.append(source);
                //     println!("from thread1 {}", thing);
                //     // // The sound plays in a separate thread. This call will block the current thread until the sink
                //     // // has finished playing all its queued sounds.
                //     sinker_ultimate.sleep_until_end();
                //     println!("from thread2 {}", thing);
                // });

                let file = BufReader::new(File::open("/home/ben/Projects/YAMP/res/sample.flac").unwrap());

                let source = Decoder::new(file).unwrap();

                // TURNS OUT I JUST HAD WRAP THIS SUCKER IN A STRUCT
                // I THINK BECAUSE THE STREAM NEEDS TO STAY ALIVE OR AUDIO WON'T PLAY
                // (_STREAM IS A FIELD IN THIS STRUCT)
                // (tested without the _stream field and audio didn't work jsyk)
                // SLEEP UNTIL END IS COMPLETELY UNNECESSARY IN THIS CASE BECAUSE
                // LIBCOSMIC KEEPS THE MAIN THREAD ALIVE
                // AND I DON'T HAVE TO MAKE A SECOND THREAD BECAUSE RODIO IS ALREADY DOING THAT
                // IN THE BACKGROUND
                // HOLY SMOKES
                self.audio_player.player.append(source);
                // WHEN YOU APPEND A SOURCE TO THE PLAYER IT IMMEDIATELY STARTS PLAYING
                // BUT IF YOU PAUSE AND APPEND ANOTHER THING IT DON'T START PLAYING AGAIN
                // THEREFORE, WE MAKE SURE TO CALL PLAY EVERY TIME.
                self.audio_player.player.play();
                //self.audio_player.player.sleep_until_end();

                println!("{}", self.thing);

                //handle.join().unwrap();


                // symphonia playback
                // thread::spawn(|| {
                //     let code = match player::gobbo() {
                //         Ok(code) => code,
                //         Err(err) => {
                //             error!("{}", err.to_string().to_lowercase());
                //             -1
                //         }
                //     };
                // });
            }

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }

                // Set the title of the context drawer.
                self.set_context_title(context_page.title());
            }
        }
        Command::none()
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<Element<Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => self.about(),
        })
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Command<Self::Message> {
        // Activate the page in the model.
        self.nav.activate(id);
        self.update_titles()
    }
}

impl Yamp {
    /// The about page for this app.
    pub fn about(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let icon = widget::svg(widget::svg::Handle::from_memory(
            &include_bytes!("../res/icons/hicolor/128x128/apps/com.example.CosmicAppTemplate.svg")
                [..],
        ));

        let title = widget::text::title3(fl!("app-title"));

        let link = widget::button::link(REPOSITORY)
            .on_press(Message::LaunchUrl(REPOSITORY.to_string()))
            .padding(0);

        widget::column()
            .push(icon)
            .push(title)
            .push(link)
            .align_items(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// Updates the header and window titles.
    pub fn update_titles(&mut self) -> Command<Message> {
        let mut window_title = fl!("app-title");
        let mut header_title = String::new();

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
            header_title.push_str(page);
        }

        self.set_header_title(header_title);
        self.set_window_title(window_title)
    }
}
