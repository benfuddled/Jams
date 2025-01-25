// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::thread;

use crate::{fl, player};
use std::fs;
use cosmic::app::{Task, Core, context_drawer};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, keyboard, Length, Subscription, time};
use cosmic::widget::{self, button, Button, Column, Container, icon, menu, nav_bar, Row, slider, text};
use cosmic::{cosmic_theme, theme, Application, ApplicationExt, Apply, Element};
use cosmic::iced_core;
use log::{error, info};

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::{Duration, Instant};
use rodio::{Decoder, OutputStream, Sink, source::Source};
use rodio::source::SineWave;

// use cosmic_files::{
//     dialog::{Dialog, DialogKind, DialogMessage, DialogResult},
//     mime_icon::{mime_for_path, mime_icon},
// };

use cosmic::dialog::file_chooser::{self, FileFilter};
use cosmic::iced_widget::{Scrollable};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{Cue, FormatOptions, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{ColorMode, MetadataOptions, MetadataRevision, Tag, Value, Visual};
use symphonia::core::probe::{Hint, ProbeResult};
use url::Url;

const REPOSITORY: &str = "https://github.com/benfuddled/YAMP";

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
    scanned_files: Vec<MusicFile>,
    thing: Arc<i32>,
    audio_player: RodioAudioPlayer,
    global_play_state: PlayState,
    current_track_duration: Duration,
    seek_position: Duration,
    last_tick: Instant,
    scrub_value: u8
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

#[derive(Clone, Debug)]
pub struct MusicFile {
    saved_path: PathBuf,
    metadata: MetadataRevision,
    playing: bool,
    paused: bool,
    track_title: String,
    track_number: u16,
    artist: String,
    album: String,
    album_artist: String,
    date: String
}

// #[derive(Default)]
// enum WatchState {
//     #[default]
//     Idle,
//     Ticking {
//         last_tick: Instant,
//     },
// }

/// This is the enum that contains all the possible variants that your application will need to transmit messages.
/// This is used to communicate between the different parts of your application.
/// If your application does not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    DebugScan,
    Play,
    Cancelled,
    CloseError,
    Error(String),
    FileRead(Url, String),
    OpenError(Arc<file_chooser::Error>),
    OpenFile,
    Selected(Url),
    AddFolder,
    AddSongsToLibrary(Url),
    StartPlayingNewTrack(PathBuf),
    PauseCurrentTrack,
    ResumeCurrentTrack,
    WatchTick(Instant),
    Scrub(u8),
}

/// Identifies a page in the application.
pub enum Page {
    Page1,
    Page2,
    Page3,
    Page4,
}

#[derive(Default)]
pub enum PlayState {
    #[default]
    Idle,
    Paused,
    Playing,
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
    DebugScan,
    OpenFile,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Play => { Message::Play }
            MenuAction::DebugScan => { Message::DebugScan }
            MenuAction::OpenFile => { Message::OpenFile }
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
    /// - `Task` type is used to send messages to your application. `Task::none()` can be used to send no messages to your application.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text("All Music")
            .data::<Page>(Page::Page1)
            .icon(icon::from_name("view-list-symbolic"))
            .activate();

        nav.insert()
            .text("Songs")
            .data::<Page>(Page::Page2)
            .icon(icon::from_name("folder-music-symbolic"));

        nav.insert()
            .text("Albums")
            .data::<Page>(Page::Page3)
            .icon(icon::from_name("view-grid-symbolic"));

        nav.insert()
            .text("Artists")
            .data::<Page>(Page::Page4)
            .icon(icon::from_name("system-users-symbolic"));

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

        let mut global_play_state: PlayState = PlayState::default();

        let mut app = Yamp {
            core,
            context_page: ContextPage::default(),
            key_binds: HashMap::new(),
            nav,
            scanned_files,
            thing,
            audio_player,
            global_play_state,
            scrub_value: 50,
            current_track_duration: Duration::default(),
            seek_position: Duration::default(),
            last_tick: Instant::now()
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
                vec![menu::Item::Button(fl!("about"), None, MenuAction::About)],
            ),
        ),
                                      menu::Tree::with_children(
                                          menu::root(fl!("debug")),
                                          menu::items(
                                              &self.key_binds,
                                              vec![menu::Item::Button(fl!("debug-play"), None, MenuAction::Play),
                                                   menu::Item::Button(fl!("debug-file-listing"), None, MenuAction::DebugScan),
                                                   menu::Item::Button(fl!("debug-file-play"), None, MenuAction::OpenFile)],
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
        let mut window_col = Column::new().spacing(10);

        // https://hermanradtke.com/2015/06/22/effectively-using-iterators-in-rust.html/
        if (&self.scanned_files.len() > &0) {
            let mut file_col = Column::new().spacing(2);

            for file in &self.scanned_files {
                //println!("Name: {}", file.saved_path.display());

                //let mut file_txt_container = Container::new(file_txt).width(Length::Fill);

                let mut file_txt_row = Row::new()
                    .align_y(Alignment::Center)
                    .spacing(5)
                    .padding([6, 4, 6, 4]);

                if (file.paused == true) {
                    //let resume_txt = text("Resume");
                    let button = button::icon(icon::from_name("media-playback-start-symbolic")).on_press(Message::ResumeCurrentTrack);
                    file_txt_row = file_txt_row.push(button);
                } else if (file.playing == true) {
                    //let playing_txt = text("Pause");
                    let button = button::icon(icon::from_name("media-playback-pause-symbolic")).on_press(Message::PauseCurrentTrack);
                    file_txt_row = file_txt_row.push(button);
                } else {
                    //let paused_txt = text("Play");
                    let button = button::icon(icon::from_name("media-playback-start-symbolic")).on_press(Message::StartPlayingNewTrack(file.saved_path.clone()));
                    file_txt_row = file_txt_row.push(button);
                }

                let title = text(file.track_title.clone()).width(Length::FillPortion(2));
                let artist = text(file.artist.clone()).width(Length::FillPortion(1));
                let album = text(file.album.clone()).width(Length::FillPortion(1));
                file_txt_row = file_txt_row.push(title);
                file_txt_row = file_txt_row.push(artist);
                file_txt_row = file_txt_row.push(album);

                file_col = file_col.push(file_txt_row);

                file_col = file_col.push(widget::divider::horizontal::default());

                // let file_txt = text(file.saved_path.display().to_string());
                // let file_txt_container = Container::new(file_txt).width(Length::Fill);
                //
                // col = col.push(file_txt_container);
            }


            let scroll_list = Scrollable::new(file_col).height(Length::Fill).width(Length::Fill);
            let scroll_container = Container::new(scroll_list).height(Length::Fill).width(Length::Fill);

            // let paused_txt = text("Play");
            // let button = button(paused_txt);

            let mut controls_row = Row::new()
                .spacing(10)
                .align_y(Alignment::Center)
                .height(Length::Fill);

            //let controls_button_prev_txt = text("Previous");
            let controls_prev_button = button::icon(icon::from_name("media-skip-backward-symbolic"))
                .icon_size(16)
                .on_press(Message::PauseCurrentTrack);

            controls_row = controls_row.push(controls_prev_button);

            match &self.global_play_state {
                PlayState::Playing => {
                    //let controls_button_txt = text("Pause");
                    let controls_pause_button = button::icon(icon::from_name("media-playback-pause-symbolic"))
                        .icon_size(24)
                        .padding([15, 15, 15, 15])
                        .class(cosmic::style::Button::Suggested)
                        .on_press(Message::PauseCurrentTrack);

                    controls_row = controls_row.push(controls_pause_button);
                }
                PlayState::Paused => {
                    //let controls_button_txt = text("Play");
                    let controls_pause_button = button::icon(icon::from_name("media-playback-start-symbolic"))
                        .icon_size(24)
                        .padding([15, 15, 15, 15])
                        .class(cosmic::style::Button::Suggested)
                        .on_press(Message::ResumeCurrentTrack);

                    controls_row = controls_row.push(controls_pause_button);
                }
                PlayState::Idle => {
                    //let controls_button_txt = text("This Button Is Disabled");
                    let controls_pause_button = button::icon(icon::from_name("media-playback-start-symbolic"))
                        .icon_size(24)
                        .padding([15, 15, 15, 15])
                        .class(cosmic::style::Button::Icon);

                    controls_row = controls_row.push(controls_pause_button);
                }
            }

            //let controls_button_next_txt = text("Next");
            let controls_next_button = button::icon(icon::from_name("media-skip-forward-symbolic"))
                .icon_size(16)
                .on_press(Message::PauseCurrentTrack);

            let controls_row = controls_row.push(controls_next_button);

            let mut controls_col = Column::new()
                .push(controls_row)
                .height(Length::Fixed(110.0))
                .width(Length::Fill)
                .align_x(Alignment::Center);

            let min_seek_position = self.seek_position.as_secs() / 60;
            let sec_seek_position = self.seek_position.as_secs() % 60;

            let min_duration = self.current_track_duration.as_secs() / 60;
            let sec_duration = self.current_track_duration.as_secs() % 60;

            //println!("{} : {}", min_seek_position, sec_seek_position);

            //let pos = self.seek_position.as_secs().to_string();
            //https://stackoverflow.com/questions/66666348/println-to-print-a-2-digit-integer
            let pos = format!("{}:{:02}", min_seek_position, sec_seek_position);
            //let total = self.current_track_duration.as_secs().to_string();
            let total = format!("{}:{:02}", min_duration, sec_duration);

            //println!("{}", self.seek_position.as_secs());

            let pos_txt = text(pos).size(18);
            let progress_scrubber = slider(0..=100, self.scrub_value, Message::Scrub).width(250);
            let total_txt = text(total).size(18);


            let timing_row = Row::new()
                .spacing(5)
                .align_y(Alignment::Center)
                .push(pos_txt)
                .push(progress_scrubber)
                .push(total_txt);


            controls_col = controls_col.push(timing_row);

            let controls_container = Container::new(controls_col)
                .class(cosmic::style::Container::ContextDrawer);

            window_col = window_col.push(scroll_container);
            window_col = window_col.push(controls_container);
        } else {
            let mut splash_screen = Column::new()
                .align_x(Alignment::Center)
                .spacing(15);

            let title = widget::text::title1(fl!("welcome"))
                .size(48)
                .apply(widget::container)
                .padding(0)
                .width(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center);

            let subtitle = text::title2(fl!("spelled-out"))
                .size(18)
                .font(cosmic::font::mono())
                .line_height(cosmic::iced_core::text::LineHeight::Relative(2.5));

            let mut titles = Column::new()
                .align_x(Alignment::Center);

            titles = titles.push(title);
            titles = titles.push(subtitle);

            splash_screen = splash_screen.push(titles);

            //let txt_open = text(fl!("add-folder")).size(20);
            // let txt_open_container = Container::new(txt_open).center_x();
            let btn_open = button::link(fl!("add-folder"))
                .font_size(20)
                .class(cosmic::style::Button::Suggested)
                .padding([16, 32])
                .on_press(Message::AddFolder);

            splash_screen = splash_screen.push(btn_open);

            let mut splash_screen_container = Row::new()
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .height(Length::Fill);

            splash_screen_container = splash_screen_container.push(splash_screen);

            window_col = window_col.push(splash_screen_container);
        }

        window_col.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let tick = match self.global_play_state {
            PlayState::Idle => Subscription::none(),
            PlayState::Paused => Subscription::none(),
            PlayState::Playing { .. } => {
                time::every(Duration::from_millis(100)).map(Message::WatchTick)
            }
        };

        fn handle_hotkey(
            key: keyboard::Key,
            _modifiers: keyboard::Modifiers,
        ) -> Option<Message> {
            use keyboard::key;

            match key.as_ref() {
                keyboard::Key::Named(key::Named::Space) => {
                    Some(Message::ResumeCurrentTrack)
                }
                keyboard::Key::Character("r") => Some(Message::PauseCurrentTrack),
                _ => None,
            }
        }

        Subscription::batch(vec![tick, keyboard::on_key_press(handle_hotkey)])
    }

    /// Application messages are handled here. The application state can be modified based on
    /// what message was received. Tasks may be returned for asynchronous execution on a
    /// background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::WatchTick(now) => {
                if let PlayState::Playing = &mut self.global_play_state {
                    self.seek_position += now - self.last_tick;
                    self.last_tick = now;

                    // update scrubber
                    self.scrub_value = (self.seek_position.as_secs() as f64 / self.current_track_duration.as_secs() as f64 * 100.0) as u8;
                }
            }
            Message::LaunchUrl(url) => {
                let _result = open::that_detached(url);
            }

            Message::DebugScan => {
                let paths = fs::read_dir("./").unwrap();

                for path in paths {
                    //println!("Name: {}", path.unwrap().path().display());
                    //self.scanned_files.push(path.unwrap().path());
                }

                self.audio_player.player.pause();

                // for file in &self.scanned_files {
                //     println!("Name: {}", file.display());
                // }
            }

            Message::AddFolder => {
                return cosmic::task::future(async move {
                    let dialog = file_chooser::open::Dialog::new().title(fl!("add-folder"));

                    match dialog.open_folder().await {
                        Ok(response) => Message::AddSongsToLibrary(response.url().to_owned()),

                        Err(file_chooser::Error::Cancelled) => Message::Cancelled,

                        Err(why) => Message::OpenError(Arc::new(why)),
                    }
                });
            }

            Message::AddSongsToLibrary(url) => {
                let paths = fs::read_dir(url.to_file_path().unwrap()).unwrap();

                for path in paths {
                    //println!("Name: {}", path.unwrap().path().display());
                    //self.scanned_files.push(path.unwrap().path());

                    let new_path = path.unwrap().path().clone();
                    let saved_path = new_path.clone();

                    // Create a hint to help the format registry guess what format reader is appropriate.
                    let mut hint = Hint::new();

                    // Open the media source.
                    let src = std::fs::File::open(new_path).expect("failed to open media");

                    // Create the media source stream.
                    let mss = MediaSourceStream::new(Box::new(src), Default::default());

                    // Use the default options for metadata and format readers.
                    let metadata_opts: MetadataOptions = Default::default();
                    let format_opts: FormatOptions = Default::default();

                    let no_progress = false;

                    // Probe the media source stream for metadata and get the format reader.
                    match symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
                        Ok(mut probed) => {
                            //print_tracks(probed.format.tracks());

                            // Prefer metadata that's provided in the container format, over other tags found during the
                            // probe operation.
                            if let Some(metadata_rev) = probed.format.metadata().current() {
                                //print_tags(metadata_rev.tags());
                                // print_visuals(metadata_rev.visuals());

                                let metadata = metadata_rev.clone();

                                let tags = metadata.tags();

                                let mut track_title = String::from("");
                                let mut album = String::from("");
                                let mut artist = String::from("");
                                let mut album_artist = String::from("");
                                let mut date = String::from("");
                                let mut track_number = 0;

                                let mut idx = 1;

                                // Print tags with a standard tag key first, these are the most common tags.
                                for tag in tags.iter().filter(|tag| tag.is_known()) {
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "TrackTitle") {
                                            track_title = tag.value.to_string();
                                            //println!("{}", &tag.value);
                                        }
                                        println!("{}", print_tag_item(idx, &format!("{:?}", std_key), &tag.value, 4));
                                    }
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "Album") {
                                            album = tag.value.to_string();
                                        }
                                    }
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "AlbumArtist") {
                                            album_artist = tag.value.to_string();
                                        }
                                    }
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "Artist") {
                                            artist = tag.value.to_string();
                                        }
                                    }
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "Date") {
                                            date = tag.value.to_string();
                                        }
                                    }
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "TrackNumber") {
                                            track_number = tag.value.to_string().parse::<u16>().unwrap();
                                        }
                                    }
                                    idx += 1;
                                }



                                let mut music_file = MusicFile {
                                    saved_path,
                                    metadata,
                                    track_title,
                                    track_number,
                                    artist,
                                    album,
                                    album_artist,
                                    playing: false,
                                    paused: false,
                                    date,
                                };

                                self.scanned_files.push(music_file);

                                // Warn that certain tags are preferred.
                                if probed.metadata.get().as_ref().is_some() {
                                    info!("tags that are part of the container format are preferentially printed.");
                                    info!("not printing additional tags that were found while probing.");
                                }
                            } else if let Some(metadata_rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
                                //print_tags(metadata_rev.tags());
                                // print_visuals(metadata_rev.visuals());

                                let metadata = metadata_rev.clone();

                                let tags = metadata.tags();

                                let mut track_title = String::from(" ");

                                let mut idx = 1;

                                // Print tags with a standard tag key first, these are the most common tags.
                                for tag in tags.iter().filter(|tag| tag.is_known()) {
                                    if let Some(std_key) = tag.std_key {
                                        if (&format!("{:?}", std_key) == "TrackTitle") {
                                            track_title = tag.value.to_string();
                                            //println!("{}", &tag.value);
                                        }
                                        println!("{}", print_tag_item(idx, &format!("{:?}", std_key), &tag.value, 4));
                                    }
                                    idx += 1;
                                }


                                // FIXME: Figure out where this condition gets its tags
                                let music_file = MusicFile {
                                    saved_path,
                                    metadata,
                                    track_title,
                                    track_number: 0,
                                    artist: "".to_string(),
                                    album: "".to_string(),
                                    album_artist: "".to_string(),
                                    playing: false,
                                    paused: false,
                                    date: "".to_string(),
                                };

                                self.scanned_files.push(music_file);
                            }
                            // print_format_sans_path(&mut probed);
                        }
                        Err(err) => {
                            // The input was not supported by any format reader.
                            info!("the input is not supported");
                        }
                    }
                }
            }

            Message::StartPlayingNewTrack(file_path) => {
                self.audio_player.player.stop();

                for file in &mut self.scanned_files {
                    file.paused = false;
                    if (file.saved_path == file_path) {
                        file.playing = true;
                    } else {
                        file.playing = false;
                    }
                }

                let file = BufReader::new(File::open(file_path).unwrap());

                let source = Decoder::new(file).unwrap();

                println!("{:?}", source.total_duration());

                self.current_track_duration = source.total_duration().unwrap();

                // Turns out I just had wrap this guy in a struct,
                // I think because the stream needs to stay alive or audio won't play.
                // (_stream is a field in this struct)
                // (tested without the _stream field and audio didn't work jsyk)
                // Sleep until end is completely unnecessary in this case because
                // libcosmic keeps the main thread alive.
                // And I don't have to make a second thread because
                // rodio is already doing that in the background
                // holy smokes
                self.audio_player.player.append(source);
                // When you append a source to the player it immediately starts playing
                // but if you pause and append another thing it don't start playing again
                // therefore, we make sure to call play every time.
                self.audio_player.player.play();

                self.last_tick = Instant::now();
                self.seek_position = Duration::default();

                self.global_play_state = PlayState::Playing;
            }

            Message::PauseCurrentTrack => {
                self.audio_player.player.pause();
                self.global_play_state = PlayState::Paused;

                for file in &mut self.scanned_files {
                    if (file.playing == true) {
                        file.playing = false;
                        file.paused = true;
                    }
                }
            }

            Message::ResumeCurrentTrack => {
                self.last_tick = Instant::now();
                self.audio_player.player.play();
                self.global_play_state = PlayState::Playing;
                for file in &mut self.scanned_files {
                    if (file.paused == true) {
                        file.playing = true;
                        file.paused = false;
                    }
                }
            }


            // Creates a new open dialog.
            // https://github.com/pop-os/libcosmic/blob/master/examples/open-dialog/src/main.rs
            Message::OpenFile => {
                return cosmic::task::future(async move {
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

                // Turns out I just had wrap this guy in a struct,
                // I think because the stream needs to stay alive or audio won't play.
                // (_stream is a field in this struct)
                // (tested without the _stream field and audio didn't work jsyk)
                // Sleep until end is completely unnecessary in this case because
                // libcosmic keeps the main thread alive.
                // And I don't have to make a second thread because
                // rodio is already doing that in the background
                // holy smokes
                self.audio_player.player.append(source);
                // When you append a source to the player it immediately starts playing
                // but if you pause and append another thing it don't start playing again
                // therefore, we make sure to call play every time.
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


                self.audio_player.player.append(source);
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
            }
            Message::Scrub(value) => {
                self.scrub_value = value;
                let percent: f64 = (f64::from(value) / 100.0);
                let pos = self.current_track_duration.as_secs() as f64 * percent;
                println!("scrub {}, pos {}, percent {}", u64::from(value), pos, percent);
                self.seek_position = Duration::from_secs(pos as u64);
                self.audio_player.player.try_seek(self.seek_position).unwrap();
                //println!("{}", value)
            }
        }
        Task::none()
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::context_drawer(
                self.about(),
                Message::ToggleContextPage(ContextPage::About),
            )
                .title(fl!("about")),
        })
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
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
            //.align_items(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// Updates the header and window titles.
    pub fn update_titles(&mut self) -> Task<Message> {
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








fn print_format_sans_path(probed: &mut ProbeResult) {
    print_tracks(probed.format.tracks());

    // Prefer metadata that's provided in the container format, over other tags found during the
    // probe operation.
    if let Some(metadata_rev) = probed.format.metadata().current() {
        // print_tags(metadata_rev.tags());
        // print_visuals(metadata_rev.visuals());

        // Warn that certain tags are preferred.
        if probed.metadata.get().as_ref().is_some() {
            info!("tags that are part of the container format are preferentially printed.");
            info!("not printing additional tags that were found while probing.");
        }
    }
    else if let Some(metadata_rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        print_tags(metadata_rev.tags());
        print_visuals(metadata_rev.visuals());
    }

    // print_cues(probed.format.cues());
    // println!(":");
    // println!();
}








fn read_media_metadata() {
    let path = Path::new("");

    // Create a hint to help the format registry guess what format reader is appropriate.
    let mut hint = Hint::new();

    // Open the media source.
    let src = std::fs::File::open(path).expect("failed to open media");

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Use the default options for metadata and format readers.
    let metadata_opts: MetadataOptions = Default::default();
    let format_opts: FormatOptions = Default::default();

    // Get the value of the track option, if provided.
    // let track = match args.value_of("track") {
    //     Some(track_str) => track_str.parse::<usize>().ok(),
    //     _ => None,
    // };

    let no_progress = false;

    // Probe the media source stream for metadata and get the format reader.
    match symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
        Ok(mut probed) => {
            // Dump visuals if requested.
            // if args.is_present("dump-visuals") {
            //     let name = match path.file_name() {
            //         Some(name) if name != "-" => name,
            //         _ => OsStr::new("NoName"),
            //     };
            //
            //     dump_visuals(&mut probed, name);
            // }

            // Select the operating mode.
            // if args.is_present("verify-only") {
            //     // Verify-only mode decodes and verifies the audio, but does not play it.
            //     decode_only(probed.format, &DecoderOptions { verify: true, ..Default::default() })
            // }
            // else if args.is_present("decode-only") {
            //     // Decode-only mode decodes the audio, but does not play or verify it.
            //     decode_only(probed.format, &DecoderOptions { verify: false, ..Default::default() })
            // }
            // else if args.is_present("probe-only") {
            // Probe-only mode only prints information about the format, tracks, metadata, etc.
            print_format(path, &mut probed);
            //}
            // else {
            //     // Playback mode.
            //     print_format(path, &mut probed);
            //
            //     // If present, parse the seek argument.
            //     let seek = if let Some(time) = args.value_of("seek") {
            //         Some(SeekPosition::Time(time.parse::<f64>().unwrap_or(0.0)))
            //     }
            //     else {
            //         args.value_of("seek-ts")
            //             .map(|ts| SeekPosition::Timetamp(ts.parse::<u64>().unwrap_or(0)))
            //     };
            //
            //     // Set the decoder options.
            //     let decode_opts =
            //         DecoderOptions { verify: args.is_present("verify"), ..Default::default() };
            //
            //     // Play it!
            //     play(probed.format, track, seek, &decode_opts, no_progress)
            // }
        }
        Err(err) => {
            // The input was not supported by any format reader.
            info!("the input is not supported");
        }
    }
}

fn print_format(path: &Path, probed: &mut ProbeResult) {
    println!("+ {}", path.display());
    print_tracks(probed.format.tracks());

    // Prefer metadata that's provided in the container format, over other tags found during the
    // probe operation.
    if let Some(metadata_rev) = probed.format.metadata().current() {
        print_tags(metadata_rev.tags());
        print_visuals(metadata_rev.visuals());

        // Warn that certain tags are preferred.
        if probed.metadata.get().as_ref().is_some() {
            info!("tags that are part of the container format are preferentially printed.");
            info!("not printing additional tags that were found while probing.");
        }
    }
    else if let Some(metadata_rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        print_tags(metadata_rev.tags());
        print_visuals(metadata_rev.visuals());
    }

    print_cues(probed.format.cues());
    println!(":");
    println!();
}

fn print_cues(cues: &[Cue]) {
    if !cues.is_empty() {
        println!("|");
        println!("| // Cues //");

        for (idx, cue) in cues.iter().enumerate() {
            println!("|     [{:0>2}] Track:      {}", idx + 1, cue.index);
            println!("|          Timestamp:  {}", cue.start_ts);

            // Print tags associated with the Cue.
            if !cue.tags.is_empty() {
                println!("|          Tags:");

                for (tidx, tag) in cue.tags.iter().enumerate() {
                    if let Some(std_key) = tag.std_key {
                        println!(
                            "{}",
                            print_tag_item(tidx + 1, &format!("{:?}", std_key), &tag.value, 21)
                        );
                    }
                    else {
                        println!("{}", print_tag_item(tidx + 1, &tag.key, &tag.value, 21));
                    }
                }
            }

            // Print any sub-cues.
            if !cue.points.is_empty() {
                println!("|          Sub-Cues:");

                for (ptidx, pt) in cue.points.iter().enumerate() {
                    println!(
                        "|                      [{:0>2}] Offset:    {:?}",
                        ptidx + 1,
                        pt.start_offset_ts
                    );

                    // Start the number of sub-cue tags, but don't print them.
                    if !pt.tags.is_empty() {
                        println!(
                            "|                           Sub-Tags:  {} (not listed)",
                            pt.tags.len()
                        );
                    }
                }
            }
        }
    }
}

fn print_tracks(tracks: &[Track]) {
    if !tracks.is_empty() {
        println!("|");
        println!("| // Tracks //");

        for (idx, track) in tracks.iter().enumerate() {
            let params = &track.codec_params;

            print!("|     [{:0>2}] Codec:           ", idx + 1);

            if let Some(codec) = symphonia::default::get_codecs().get_codec(params.codec) {
                println!("{} ({})", codec.long_name, codec.short_name);
            }
            else {
                println!("Unknown (#{})", params.codec);
            }

            // if let Some(sample_rate) = params.sample_rate {
            //     println!("|          Sample Rate:     {}", sample_rate);
            // }
            // if params.start_ts > 0 {
            //     if let Some(tb) = params.time_base {
            //         println!(
            //             "|          Start Time:      {} ({})",
            //             fmt_time(params.start_ts, tb),
            //             params.start_ts
            //         );
            //     }
            //     else {
            //         println!("|          Start Time:      {}", params.start_ts);
            //     }
            // }
            // if let Some(n_frames) = params.n_frames {
            //     if let Some(tb) = params.time_base {
            //         println!(
            //             "|          Duration:        {} ({})",
            //             fmt_time(n_frames, tb),
            //             n_frames
            //         );
            //     }
            //     else {
            //         println!("|          Frames:          {}", n_frames);
            //     }
            // }
            // if let Some(tb) = params.time_base {
            //     println!("|          Time Base:       {}", tb);
            // }
            // if let Some(padding) = params.delay {
            //     println!("|          Encoder Delay:   {}", padding);
            // }
            // if let Some(padding) = params.padding {
            //     println!("|          Encoder Padding: {}", padding);
            // }
            // if let Some(sample_format) = params.sample_format {
            //     println!("|          Sample Format:   {:?}", sample_format);
            // }
            // if let Some(bits_per_sample) = params.bits_per_sample {
            //     println!("|          Bits per Sample: {}", bits_per_sample);
            // }
            // if let Some(channels) = params.channels {
            //     println!("|          Channel(s):      {}", channels.count());
            //     println!("|          Channel Map:     {}", channels);
            // }
            // if let Some(channel_layout) = params.channel_layout {
            //     println!("|          Channel Layout:  {:?}", channel_layout);
            // }
            // if let Some(language) = &track.language {
            //     println!("|          Language:        {}", language);
            // }
        }
    }
}

fn print_visuals(visuals: &[Visual]) {
    if !visuals.is_empty() {
        println!("|");
        println!("| // Visuals //");

        for (idx, visual) in visuals.iter().enumerate() {
            if let Some(usage) = visual.usage {
                println!("|     [{:0>2}] Usage:      {:?}", idx + 1, usage);
                println!("|          Media Type: {}", visual.media_type);
            }
            else {
                println!("|     [{:0>2}] Media Type: {}", idx + 1, visual.media_type);
            }
            if let Some(dimensions) = visual.dimensions {
                println!(
                    "|          Dimensions: {} px x {} px",
                    dimensions.width, dimensions.height
                );
            }
            if let Some(bpp) = visual.bits_per_pixel {
                println!("|          Bits/Pixel: {}", bpp);
            }
            if let Some(ColorMode::Indexed(colors)) = visual.color_mode {
                println!("|          Palette:    {} colors", colors);
            }
            println!("|          Size:       {} bytes", visual.data.len());

            // Print out tags similar to how regular tags are printed.
            if !visual.tags.is_empty() {
                println!("|          Tags:");
            }

            for (tidx, tag) in visual.tags.iter().enumerate() {
                if let Some(std_key) = tag.std_key {
                    println!(
                        "{}",
                        print_tag_item(tidx + 1, &format!("{:?}", std_key), &tag.value, 21)
                    );
                }
                else {
                    println!("{}", print_tag_item(tidx + 1, &tag.key, &tag.value, 21));
                }
            }
        }
    }
}

fn print_tags(tags: &[Tag]) {
    if !tags.is_empty() {
        println!("|");
        println!("| // Tags //");

        let mut idx = 1;

        // Print tags with a standard tag key first, these are the most common tags.
        for tag in tags.iter().filter(|tag| tag.is_known()) {
            if let Some(std_key) = tag.std_key {
                println!("{}", print_tag_item(idx, &format!("{:?}", std_key), &tag.value, 4));
            }
            idx += 1;
        }

        // Print the remaining tags with keys truncated to 26 characters.
        for tag in tags.iter().filter(|tag| !tag.is_known()) {
            println!("{}", print_tag_item(idx, &tag.key, &tag.value, 4));
            idx += 1;
        }
    }
}

fn print_tag_item(idx: usize, key: &str, value: &Value, indent: usize) -> String {
    let key_str = match key.len() {
        0..=28 => format!("| {:w$}[{:0>2}] {:<28} : ", "", idx, key, w = indent),
        _ => format!("| {:w$}[{:0>2}] {:.<28} : ", "", idx, key.split_at(26).0, w = indent),
    };

    let line_prefix = format!("\n| {:w$} : ", "", w = indent + 4 + 28 + 1);
    let line_wrap_prefix = format!("\n| {:w$}   ", "", w = indent + 4 + 28 + 1);

    let mut out = String::new();

    out.push_str(&key_str);

    for (wrapped, line) in value.to_string().lines().enumerate() {
        if wrapped > 0 {
            out.push_str(&line_prefix);
        }

        let mut chars = line.chars();
        let split = (0..)
            .map(|_| chars.by_ref().take(72).collect::<String>())
            .take_while(|s| !s.is_empty())
            .collect::<Vec<_>>();

        out.push_str(&split.join(&line_wrap_prefix));
    }

    out
}