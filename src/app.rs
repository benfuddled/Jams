// SPDX-License-Identifier: GPL-3.0-only

use crate::fl;
use cosmic::app::{context_drawer, Core, Task};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{keyboard, time, Alignment, Length, Subscription};
use cosmic::widget::{self, button, icon, menu, nav_bar, slider, text, Column, Container, Row};
use cosmic::{cosmic_theme, theme, Application, ApplicationExt, Apply, Element};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use infer::Infer;

use crate::icon_cache::IconCache;
use cosmic::dialog::file_chooser::{self};
use cosmic::iced_widget::Scrollable;
use url::Url;
use walkdir::WalkDir;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::{glib, ClockTime};
use gstreamer_pbutils::{prelude::*, DiscovererResult};
use gstreamer_play as gst_play;

const REPOSITORY: &str = "https://github.com/benfuddled/Jams";

lazy_static::lazy_static! {
    static ref ICON_CACHE: Mutex<IconCache> = Mutex::new(IconCache::new());
}

pub fn icon_cache_get(name: &'static str, size: u16) -> widget::icon::Icon {
    let mut icon_cache = ICON_CACHE.lock().unwrap();
    icon_cache.get(name, size)
}

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
pub struct Jams {
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
    alt_player: GStreamerPlayer,
    global_play_state: PlayState,
    current_track_duration: Duration,
    seek_position: Duration,
    last_tick: Instant,
    scrub_value: u8,
}

pub struct GStreamerPlayer {
    /// The sink responsible for managing the audio playback.
    player: gst_play::Play,
    /// Store content for rewind/replay
    content: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct MusicFile {
    saved_path: PathBuf,
    uri: String,
    playing: bool,
    paused: bool,
    track_title: String,
    track_number: u16,
    duration: Duration,
    artist: String,
    album: String,
    album_artist: String,
    date: String,
}

#[derive(Clone, Debug)]
pub struct GSTWrapper {
    pipeline: gst::Pipeline,
}

/// This is the enum that contains all the possible variants that your application will need to transmit messages.
/// This is used to communicate between the different parts of your application.
/// If your application does not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    Cancelled,
    CloseError,
    Error(String),
    FileRead(Url, String),
    OpenError(Arc<file_chooser::Error>),
    AddFolder,
    AddSongsToLibrary(Url),
    StartPlayingNewTrack(String),
    PauseCurrentTrack,
    ResumeCurrentTrack,
    WatchTick(Instant),
    Scrub(u8),
    SkipNext,
    SkipPrev,
    DebugStub,
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
    DebugStub,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::DebugStub => Message::DebugStub,
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
impl Application for Jams {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "com.benfuddled.Jams";

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
            .icon(icon_cache_get("music-note-symbolic", 16))
            .activate();

        nav.insert()
            .text("Songs")
            .data::<Page>(Page::Page2)
            .icon(icon_cache_get("music-note-single-symbolic", 16));

        nav.insert()
            .text("Albums")
            .data::<Page>(Page::Page3)
            .icon(icon_cache_get("library-music-symbolic", 16));

        nav.insert()
            .text("Artists")
            .data::<Page>(Page::Page4)
            .icon(icon_cache_get("music-artist-symbolic", 16));

        let scanned_files = vec![];

        gst::init().expect("Could not initialize GStreamer.");

        let play = gst_play::Play::new(None::<gst_play::PlayVideoRenderer>);
        let gst_content = Vec::new();

        let mut alt_player = GStreamerPlayer {
            player: play,
            content: gst_content,
        };

        let mut global_play_state: PlayState = PlayState::default();

        let mut app = Jams {
            core,
            context_page: ContextPage::default(),
            key_binds: HashMap::new(),
            nav,
            scanned_files,
            alt_player,
            global_play_state,
            scrub_value: 50,
            current_track_duration: Duration::default(),
            seek_position: Duration::default(),
            last_tick: Instant::now(),
        };

        let command = app.update_titles();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let menu_bar = menu::bar(vec![
            menu::Tree::with_children(
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
                    vec![menu::Item::Button(
                        fl!("debug"),
                        None,
                        MenuAction::DebugStub,
                    )],
                ),
            ),
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
        if &self.scanned_files.len() > &0 {
            let mut file_col = Column::new().spacing(2);

            for file in &self.scanned_files {
                //println!("Name: {}", file.saved_path.display());

                //let mut file_txt_container = Container::new(file_txt).width(Length::Fill);

                let mut file_txt_row = Row::new()
                    .align_y(Alignment::Center)
                    .spacing(5)
                    .padding([6, 4, 6, 4]);

                if file.paused == true {
                    //let resume_txt = text("Resume");
                    let button = button::icon(icon::from_name("media-playback-start-symbolic"))
                        .on_press(Message::ResumeCurrentTrack);
                    file_txt_row = file_txt_row.push(button);
                } else if file.playing == true {
                    //let playing_txt = text("Pause");
                    let button = button::icon(icon::from_name("media-playback-pause-symbolic"))
                        .on_press(Message::PauseCurrentTrack);
                    file_txt_row = file_txt_row.push(button);
                } else {
                    //let paused_txt = text("Play");
                    let button = button::icon(icon::from_name("media-playback-start-symbolic"))
                        .on_press(Message::StartPlayingNewTrack(file.uri.clone()));
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

            let scroll_list = Scrollable::new(file_col)
                .height(Length::Fill)
                .width(Length::Fill);
            let scroll_container = Container::new(scroll_list)
                .height(Length::Fill)
                .width(Length::Fill);

            // let paused_txt = text("Play");
            // let button = button(paused_txt);

            let mut controls_row = Row::new()
                .spacing(10)
                .align_y(Alignment::Center)
                .height(Length::Fill);

            //let controls_button_prev_txt = text("Previous");
            let controls_prev_button =
                button::icon(icon::from_name("media-skip-backward-symbolic"))
                    .icon_size(16)
                    .on_press(Message::SkipPrev);

            controls_row = controls_row.push(controls_prev_button);

            match &self.global_play_state {
                PlayState::Playing => {
                    //let controls_button_txt = text("Pause");
                    let controls_pause_button =
                        button::icon(icon::from_name("media-playback-pause-symbolic"))
                            .icon_size(24)
                            .padding([15, 15, 15, 15])
                            .class(cosmic::style::Button::Suggested)
                            .on_press(Message::PauseCurrentTrack);

                    controls_row = controls_row.push(controls_pause_button);
                }
                PlayState::Paused => {
                    //let controls_button_txt = text("Play");
                    let controls_pause_button =
                        button::icon(icon::from_name("media-playback-start-symbolic"))
                            .icon_size(24)
                            .padding([15, 15, 15, 15])
                            .class(cosmic::style::Button::Suggested)
                            .on_press(Message::ResumeCurrentTrack);

                    controls_row = controls_row.push(controls_pause_button);
                }
                PlayState::Idle => {
                    //let controls_button_txt = text("This Button Is Disabled");
                    let controls_pause_button =
                        button::icon(icon::from_name("media-playback-start-symbolic"))
                            .icon_size(24)
                            .padding([15, 15, 15, 15])
                            .class(cosmic::style::Button::Icon);

                    controls_row = controls_row.push(controls_pause_button);
                }
            }

            //let controls_button_next_txt = text("Next");
            let controls_next_button = button::icon(icon::from_name("media-skip-forward-symbolic"))
                .icon_size(16)
                .on_press(Message::SkipNext);

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

            let controls_container =
                Container::new(controls_col).class(cosmic::style::Container::ContextDrawer);

            window_col = window_col.push(scroll_container);
            window_col = window_col.push(controls_container);
        } else {
            let mut splash_screen = Column::new().align_x(Alignment::Center).spacing(15);

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

            let mut titles = Column::new().align_x(Alignment::Center);

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

        fn handle_hotkey(key: keyboard::Key, _modifiers: keyboard::Modifiers) -> Option<Message> {
            use keyboard::key;

            match key.as_ref() {
                keyboard::Key::Named(key::Named::Space) => Some(Message::ResumeCurrentTrack),
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
                    self.scrub_value = (self.seek_position.as_secs() as f64
                        / self.current_track_duration.as_secs() as f64
                        * 100.0) as u8;

                    if self.seek_position.as_millis() >= self.current_track_duration.as_millis() {
                        println!("{}", String::from("End of track reached."));
                        self.global_play_state = PlayState::Idle;

                        let next_index = self
                            .scanned_files
                            .iter()
                            .position(|x| x.playing == true)
                            .unwrap()
                            + 1;

                        let next_file = self.scanned_files.get(next_index);

                        match next_file {
                            Some(track) => {
                                println!("Moving to next track: {}", track.track_title);
                                self.seek_position = Duration::new(0, 0);
                                self.alt_player.player.stop();
                                self.global_play_state = PlayState::Idle;
                                self.current_track_duration = Duration::new(0, 0);
                                self.switch_track(track.uri.clone());
                            }
                            None => {
                                println!("End of list reached. Stopping playback.");
                                self.seek_position = Duration::new(0, 0);
                                self.alt_player.player.stop();
                                self.global_play_state = PlayState::Idle;
                                self.current_track_duration = Duration::new(0, 0);
                            }
                        }
                        //Message::StartPlayingNewTrack();
                    }
                }
            }
            Message::SkipNext => {
                let next_index = self
                    .scanned_files
                    .iter()
                    .position(|x| x.playing == true || x.paused == true)
                    .unwrap()
                    + 1;

                let next_file = self.scanned_files.get(next_index);

                match next_file {
                    Some(track) => {
                        println!("Moving to next track: {}", track.track_title);
                        self.seek_position = Duration::new(0, 0);
                        self.alt_player.player.stop();
                        self.global_play_state = PlayState::Idle;
                        self.current_track_duration = Duration::new(0, 0);
                        self.switch_track(track.uri.clone());
                    }
                    None => {
                        println!("End of list reached. Stopping playback.");
                        self.seek_position = Duration::new(0, 0);
                        self.alt_player.player.stop();
                        self.global_play_state = PlayState::Idle;
                        self.current_track_duration = Duration::new(0, 0);
                    }
                }
            }
            Message::SkipPrev => {
                let curr_index = self
                    .scanned_files
                    .iter()
                    .position(|x| x.playing == true || x.paused == true);

                match curr_index {
                    Some(index) => {
                        if index == 0 {
                            self.scrub(0);
                        } else {
                            let prev_file = self.scanned_files.get(index - 1);

                            match prev_file {
                                Some(track) => {
                                    println!("Moving to prev track: {}", track.track_title);
                                    self.seek_position = Duration::new(0, 0);
                                    self.global_play_state = PlayState::Idle;
                                    self.current_track_duration = Duration::new(0, 0);
                                    self.switch_track(track.uri.clone());
                                }
                                None => {
                                    println!("End of list reached. Stopping playback.");
                                    self.seek_position = Duration::new(0, 0);
                                    self.alt_player.player.stop();
                                    self.global_play_state = PlayState::Idle;
                                    self.current_track_duration = Duration::new(0, 0);
                                }
                            }
                        }
                    }
                    None => {
                        println!("Can't move to previous track. No track currently playing.");
                    }
                }
            }
            Message::LaunchUrl(url) => {
                let _result = open::that_detached(url);
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

                let loop_ = glib::MainLoop::new(None, false);
                let timeout = 5 * gst::ClockTime::SECOND;
                let discoverer = gstreamer_pbutils::Discoverer::new(timeout).unwrap();

                for entry in WalkDir::new(url.to_file_path().unwrap())
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let is_audio = is_audio_file(entry.path()).unwrap_or_else(|err| false);
                    if entry.file_type().is_file() && is_audio {
                        let saved_path = entry.clone().into_path();
                        match Url::from_file_path(entry.into_path().clone()) {
                            Ok(url) => {
                                println!("url {}", url);

                                let mut track_title = String::from("");
                                let mut album = String::from("");
                                let mut artist = String::from("");
                                let mut album_artist = String::from("");
                                let mut date = String::from("");
                                let mut track_number = 0;
                                let mut duration = Duration::default();

                                match discoverer.discover_uri(url.as_str()) {
                                    Err(err) => {
                                        println!("{:?}", err);
                                    }
                                    Ok(info) => {
                                        match info.result() {
                                            DiscovererResult::Ok => println!("Discovered {url}"),
                                            DiscovererResult::UriInvalid => {
                                                println!("Invalid uri {url}")
                                            }
                                            DiscovererResult::Error => {
                                                println!("Error in DiscovererResult")
                                                // if let Some(msg) = error {
                                                //     println!("{msg}");
                                                // } else {
                                                //     println!("Unknown error")
                                                // }
                                            }
                                            DiscovererResult::Timeout => println!("Timeout"),
                                            DiscovererResult::Busy => println!("Busy"),
                                            DiscovererResult::MissingPlugins => {
                                                if let Some(s) = info.misc() {
                                                    println!("{s}");
                                                }
                                            }
                                            _ => println!("Unknown result"),
                                        }

                                        if info.result() == DiscovererResult::Ok {
                                            match info.duration() {
                                                None => {}
                                                Some(time) => {
                                                    duration = Duration::from(time);
                                                }
                                            };
                                            if let Some(tags) = info.tags() {
                                                println!("Tags:");
                                                for (tag, values) in tags.iter_generic() {
                                                    // println!("{:?}", values);
                                                    // print!("  {tag}: ");
                                                    if tag == "title" {
                                                        values.for_each(|v| {
                                                            if let Some(s) = send_value_as_str(v) {
                                                                track_title = s;
                                                            }
                                                        })
                                                    } else if tag == "album" {
                                                        values.for_each(|v| {
                                                            if let Some(s) = send_value_as_str(v) {
                                                                album = s;
                                                            }
                                                        })
                                                    } else if tag == "artist" {
                                                        values.for_each(|v| {
                                                            if let Some(s) = send_value_as_str(v) {
                                                                artist = s;
                                                            }
                                                        })
                                                    } else if tag == "album-artist" {
                                                        values.for_each(|v| {
                                                            if let Some(s) = send_value_as_str(v) {
                                                                album_artist = s;
                                                            }
                                                        })
                                                    } else if tag == "datetime" {
                                                        values.for_each(|v| {
                                                            if let Some(s) = send_value_as_str(v) {
                                                                date = s;
                                                            }
                                                        })
                                                    } else if tag == "track-number" {
                                                        // values.for_each(|v| {
                                                        //     match v.to_string().parse::<u16>() {
                                                        //         Ok(num) => {
                                                        //             track_number = num;
                                                        //         }
                                                        //         Err(err) => {
                                                        //             //println!("Track {} invalid. Assigning 0. {}", v, err);
                                                        //             track_number = 0;
                                                        //         }
                                                        //     }
                                                        //     // if let Some(s) = send_value_as_str(v) {
                                                        //     //     track_number = s;
                                                        //     // }
                                                        // })
                                                    }
                                                }
                                            }
                                        }

                                        let music_file = MusicFile {
                                            saved_path: saved_path.clone(),
                                            uri: url.to_string(),
                                            //metadata,
                                            track_title,
                                            track_number,
                                            artist,
                                            album,
                                            album_artist,
                                            duration,
                                            playing: false,
                                            paused: false,
                                            date,
                                        };

                                        self.scanned_files.push(music_file);
                                    }
                                }
                            }
                            Err(err) => eprintln!("Failed to run discovery: {err:?}"),
                        }
                    }
                }
            }

            Message::StartPlayingNewTrack(uri) => {
                self.switch_track(uri);
            }

            Message::PauseCurrentTrack => {
                self.alt_player.player.pause();
                //self.audio_player.player.pause();
                self.global_play_state = PlayState::Paused;

                for file in &mut self.scanned_files {
                    if file.playing == true {
                        file.playing = false;
                        file.paused = true;
                    }
                }
            }

            Message::ResumeCurrentTrack => {
                self.last_tick = Instant::now();
                self.alt_player.player.play();
                //self.audio_player.player.play();
                self.global_play_state = PlayState::Playing;
                for file in &mut self.scanned_files {
                    if file.paused == true {
                        file.playing = true;
                        file.paused = false;
                    }
                }
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
                self.scrub(value);
            }
            Message::DebugStub => {
                println!("This doesn't do anything right now.");
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

impl Jams {
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

    pub fn switch_track(&mut self, uri: String) {
        self.alt_player.player.stop();

        for file in &mut self.scanned_files {
            file.paused = false;
            if file.uri == uri {
                file.playing = true;
                self.current_track_duration = file.duration;
            } else {
                file.playing = false;
            }
        }

        self.alt_player.player.set_uri(Some(uri.as_str()));

        self.alt_player.player.play();

        self.last_tick = Instant::now();
        self.seek_position = Duration::default();

        self.global_play_state = PlayState::Playing;
    }

    pub fn scrub(&mut self, value: u8) {
        self.scrub_value = value;
        let percent: f64 = f64::from(value) / 100.0;
        let pos = self.current_track_duration.as_secs() as f64 * percent;
        println!(
            "scrub {}, pos {}, percent {}",
            u64::from(value),
            pos,
            percent
        );
        self.seek_position = Duration::from_secs(pos as u64);
        self.alt_player
            .player
            .seek(ClockTime::from_seconds(pos as u64));
    }
}

fn is_audio_file(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buf = [0; 1024]; // Read first KB for detection
    file.read_exact(&mut buf)?;

    let info = Infer::new();
    Ok(info.is_audio(&buf))
}

fn send_value_as_str(v: &glib::SendValue) -> Option<String> {
    if let Ok(s) = v.get::<&str>() {
        Some(s.to_string())
    } else if let Ok(serialized) = v.serialize() {
        Some(serialized.into())
    } else {
        None
    }
}
