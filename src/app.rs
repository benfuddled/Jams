// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use crate::fl;
use cosmic::app::{context_drawer, Core, Task};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{alignment, keyboard, time, Alignment, ContentFit, Length, Subscription};
use cosmic::widget::{self, button, icon, image, menu, nav_bar, slider, text, Column, Container, FlexRow, Grid, Row};
use cosmic::{cosmic_theme, theme, Application, ApplicationExt, Apply, Element};
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::tag::ItemKey;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use std::fs::File;
use std::io::{Read, Write};
use std::ops::Deref;
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
use gstreamer_play as gst_play;
use lofty::picture::Picture;

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
    albums: Vec<Album>,
    audio_player: GStreamerPlayer,
    global_play_state: PlayState,
    current_track_duration: Duration,
    seek_position: Duration,
    last_tick: Instant,
    scrub_value: u8,
    search_expanded: bool,
    search_term: String,
}

pub struct GStreamerPlayer {
    /// The sink responsible for managing the audio playback.
    player: gst_play::Play,
    /// Store content for rewind/replay
    content: Vec<u8>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct MusicFile {
    album_artist: String,
    album: String,
    track_number: u16,
    artist: String,
    track_title: String,
    duration: Duration,
    date: String,
    saved_path: PathBuf,
    uri: String,
    playing: bool,
    paused: bool,
    id: usize,
}

#[derive(Debug, Clone)]
pub struct Album {
    album_artist: String,
    album: String,
    cached_cover_path: String,
    tracks: Vec<usize>, // TODO: refactor to use arc
}

// TODO: MAKE THESE SOME()
impl Default for MusicFile {
    fn default() -> Self {
        MusicFile {
            saved_path: PathBuf::new(),
            uri: "/uri-does-not-exist".to_string(),
            playing: false,
            paused: false,
            track_title: "Invalid Title".to_string(),
            track_number: 0,
            duration: Duration::new(0, 0),
            artist: "Invalid Artist".to_string(),
            album: "Invalid Album".to_string(),
            album_artist: "Invalid Album Artist".to_string(),
            date: "Invalid Date".to_string(),
            id: 0,
        }
    }
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
    SearchExpand,
    SearchInput(String),
    DebugStub,
    SearchMinimize,
    SaveLibraryLocation,
    ResetLibraryLocation,
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
    SaveLibraryLocation,
    ResetLibraryLocation,
    ReOpenLibraryLocation,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::DebugStub => Message::DebugStub,
            MenuAction::SaveLibraryLocation => Message::SaveLibraryLocation,
            MenuAction::ResetLibraryLocation => Message::ResetLibraryLocation,
            MenuAction::ReOpenLibraryLocation => match fs::read_to_string("~/.config/jams/locations") {
                    Ok(contents) => {
                        println!("Locations contents: {}", contents);
                        let path = Path::new(contents.as_str());
                        if path.exists() {
                            match Url::from_file_path(path) {
                                Ok(url) => Message::AddSongsToLibrary(url),
                                Err(_) => {
                                    println!("Failed to convert library path to URL");
                                    Message::DebugStub
                                }
                            }
                        } else {
                            Message::DebugStub
                        }
                    }
                    Err(_) => {
                        println!("Failed to open library config.");
                        Message::DebugStub
                    }
                }
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
        let albums = vec![];

        gst::init().expect("Could not initialize GStreamer.");

        let play = gst_play::Play::new(None::<gst_play::PlayVideoRenderer>);
        let gst_content = Vec::new();

        let mut audio_player = GStreamerPlayer {
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
            albums,
            audio_player,
            global_play_state,
            scrub_value: 50,
            current_track_duration: Duration::default(),
            seek_position: Duration::default(),
            last_tick: Instant::now(),
            search_expanded: false,
            search_term: "".to_string(),
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
                    ),
                         menu::Item::Button(
                             "Save Library Location".to_string(),
                             None,
                             MenuAction::SaveLibraryLocation,
                         ),
                    menu::Item::Button(
                        "Reset Library Location".to_string(),
                        None,
                        MenuAction::ResetLibraryLocation,
                    ),
                    menu::Item::Button(
                        "Re-Open Library Location".to_string(),
                        None,
                        MenuAction::ReOpenLibraryLocation,
                    )],
                ),
            ),
        ]);

        vec![menu_bar.into()]
    }

    fn header_end(&self) -> Vec<Element<Self::Message>> {
        let mut elements = Vec::with_capacity(1);

        if self.search_expanded {
            elements.push(
                widget::text_input::search_input("Search", &self.search_term)
                    .width(Length::Fixed(240.0))
                    .on_clear(Message::SearchMinimize)
                    .always_active()
                    //.id(self.search_id.clone())
                    .on_input(Message::SearchInput)
                    .into(),
            );
        } else {
            elements.push(
                widget::button::icon(icon::from_name("system-search-symbolic"))
                    .on_press(Message::SearchExpand)
                    .padding(8)
                    .selected(true)
                    .into(),
            );
        }

        elements
    }

    /// This is the main view of your application, it is the root of your widget tree.
    ///
    /// The `Element` type is used to represent the visual elements of your application,
    /// it has a `Message` associated with it, which dictates what type of message it can send.
    ///
    /// To get a better sense of which widgets are available, check out the `widget` module.
    fn view(&self) -> Element<Self::Message> {
        // self.nav.text() - pass it a nav item from the model to get its text
        println!("{:?}", self.nav.active()); // - get currently active nav
                                             // println!("{:?}", self
                                             //     .nav
                                             //     .active_data::<String>()
                                             //     .map_or("No page selected", String::as_str));
        println!("{:?}", self.nav.text(self.nav.active()));
        let mut window_col = Column::new().spacing(10);

        // https://hermanradtke.com/2015/06/22/effectively-using-iterators-in-rust.html/
        if &self.scanned_files.len() > &0 {
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

            // TODO: Improve performance when rendering pages (specifically switching between them)
            if self.nav.text(self.nav.active()) == Option::from("All Music") {
                let mut file_col = Column::new().spacing(2);

                for file in &self.scanned_files {
                    if self.search_term.is_empty()
                        || file
                            .album
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                        || file
                            .artist
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                        || file
                            .track_title
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                        || file
                            .album_artist
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                    {
                        let mut file_txt_row = Row::new()
                            .align_y(Alignment::Center)
                            .spacing(8)
                            .padding([6, 4, 6, 4]);

                        let track_number = text(file.track_number.to_string())
                            .align_x(Horizontal::Center)
                            .width(Length::FillPortion(1));
                        file_txt_row = file_txt_row.push(track_number);

                        if file.paused == true {
                            //let resume_txt = text("Resume");
                            let button =
                                button::icon(icon::from_name("media-playback-start-symbolic"))
                                    .on_press(Message::ResumeCurrentTrack);
                            file_txt_row = file_txt_row.push(button);
                        } else if file.playing == true {
                            //let playing_txt = text("Pause");
                            let button =
                                button::icon(icon::from_name("media-playback-pause-symbolic"))
                                    .on_press(Message::PauseCurrentTrack);
                            file_txt_row = file_txt_row.push(button);
                        } else {
                            //let paused_txt = text("Play");
                            let button =
                                button::icon(icon::from_name("media-playback-start-symbolic"))
                                    .on_press(Message::StartPlayingNewTrack(file.uri.clone()));
                            file_txt_row = file_txt_row.push(button);
                        }

                        let title = text(file.track_title.clone()).width(Length::FillPortion(40));
                        let artist = text(file.artist.clone()).width(Length::FillPortion(20));
                        let album = text(file.album.clone()).width(Length::FillPortion(20));
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
                }

                let scroll_list = Scrollable::new(file_col)
                    .height(Length::Fill)
                    .width(Length::Fill);
                let scroll_container = Container::new(scroll_list)
                    .height(Length::Fill)
                    .width(Length::Fill);

                // let paused_txt = text("Play");
                // let button = button(paused_txt);

                window_col = window_col.push(scroll_container);
            } else if self.nav.text(self.nav.active()) == Option::from("Albums") {

                let mut list_of_albums = Row::new().width(Length::Fill).align_y(Alignment::Center);

                for album in &self.albums {
                    if self.search_term.is_empty()
                        || album
                            .album
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                        || album
                            .album_artist
                            .to_lowercase()
                            .contains(&self.search_term.to_lowercase())
                    {
                        let mut album_content = Column::new();

                        let album_front_cover = image(album.cached_cover_path.clone()).width(Length::Fixed(270.0)).height(Length::Fixed(270.0)).content_fit(ContentFit::Contain);
                        let album_name = text(album.album.clone()).width(Length::Fill).align_x(Alignment::Center);

                        album_content = album_content.push(album_front_cover);
                        album_content = album_content.push(album_name);

                        let mut album_content_alignment = Row::new().align_y(Alignment::Start);
                        album_content_alignment = album_content_alignment.push(album_content);

                        let mut album_block = Column::new()
                            .width(Length::Fill)
                            .max_width(300)
                            .spacing(8)
                            .padding([6, 4, 6, 4]);
                        album_block = album_block.push(album_content_alignment);

                        list_of_albums = list_of_albums.push(album_block);
                    }
                }

                let list_of_albums_wrapped = list_of_albums.wrap();

                let scroll_list = Scrollable::new(list_of_albums_wrapped)
                    .height(Length::Fill)
                    .width(Length::Fill);
                let scroll_container = Container::new(scroll_list)
                    .height(Length::Fill)
                    .width(Length::Fill);

                window_col = window_col.push(scroll_container);
            }
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
                                self.audio_player.player.stop();
                                self.global_play_state = PlayState::Idle;
                                self.current_track_duration = Duration::new(0, 0);
                                self.switch_track(track.uri.clone());
                            }
                            None => {
                                println!("End of list reached. Stopping playback.");
                                self.seek_position = Duration::new(0, 0);
                                self.audio_player.player.stop();
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
                        self.audio_player.player.stop();
                        self.global_play_state = PlayState::Idle;
                        self.current_track_duration = Duration::new(0, 0);
                        self.switch_track(track.uri.clone());
                    }
                    None => {
                        println!("End of list reached. Stopping playback.");
                        self.seek_position = Duration::new(0, 0);
                        self.audio_player.player.stop();
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
                                    self.audio_player.player.stop();
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
                // for entry in WalkDir::new(url.to_file_path().unwrap())
                //     .into_iter()
                //     .filter_map(|e| e.ok())
                // {
                for (index, entry) in WalkDir::new(url.to_file_path().unwrap())
                    .into_iter()
                    .enumerate()
                {
                    match entry {
                        Ok(entry) => {
                            let is_audio = is_audio_file(entry.path()).unwrap_or_else(|_| false);

                            if entry.file_type().is_file() && is_audio {
                                let saved_path = entry.clone().into_path();
                                println!("{}", entry.path().display());
                                match Url::from_file_path(entry.clone().into_path()) {
                                    Ok(url) => {
                                        let tagged_file =
                                            match lofty::read_from_path(entry.clone().path()) {
                                                Ok(file) => file,
                                                Err(err) => {
                                                    eprintln!("Error reading file: {}", err);
                                                    continue;
                                                }
                                            };

                                        if let Some(tag) = tagged_file.primary_tag() {
                                            let track_title = match tag
                                                .get_string(&ItemKey::TrackTitle)
                                                .map(|s| s.to_string())
                                            {
                                                Some(title) => title,
                                                None => {
                                                    // If there's no track tag, fall back to the file name.
                                                    match entry.path().file_name() {
                                                        Some(filename) => match filename.to_str() {
                                                            Some(filename) => filename.to_string(),
                                                            None => String::from(""),
                                                        },
                                                        None => String::from(""),
                                                    }
                                                }
                                            };
                                            let album = tag
                                                .album()
                                                .map(|s| s.to_string())
                                                .unwrap_or_else(|| String::from("Unknown Album"));
                                            let artist = tag
                                                .artist()
                                                .map(|s| s.to_string())
                                                .unwrap_or_default();
                                            let album_artist = match tag
                                                .get_string(&ItemKey::AlbumArtist)
                                                .map(|s| s.to_string())
                                            {
                                                Some(album_artist) => album_artist,
                                                None => artist.clone(),
                                            };
                                            let date = tag
                                                .year()
                                                .map(|s| s.to_string())
                                                .unwrap_or_default();
                                            let track_number = match tag
                                                .track()
                                                .map(|s| s.to_string())
                                            {
                                                Some(track) => track.parse::<u16>().unwrap_or(0),
                                                None => 0,
                                            };

                                            let properties =
                                                lofty::prelude::AudioFile::properties(&tagged_file);
                                            let duration = Duration::from_secs(
                                                properties.duration().as_secs(),
                                            );

                                            // println!("{}", tag.picture_count());
                                            // let thing = tag.pictures();
                                            // for pic in tag.pictures() {
                                            //     println!("{:?}", pic.pic_type());
                                            // }

                                            let music_file = MusicFile {
                                                album_artist: album_artist.clone(),
                                                album: album.clone(),
                                                track_number,
                                                artist,
                                                track_title,
                                                duration,
                                                date,
                                                saved_path: saved_path.clone(),
                                                uri: url.to_string(),
                                                //metadata,
                                                playing: false,
                                                paused: false,
                                                id: index,
                                            };

                                            match self.albums.iter_mut().find(|album| {
                                                album.album == music_file.album
                                                    && album.album_artist == music_file.album_artist
                                            }) {
                                                Some(album) => {
                                                    album.tracks.push(index);
                                                }
                                                None => {

                                                    let path_to_write = "~/.local/share/jams/covers/".to_string() + index.to_string().as_str();

                                                    match tag.pictures().first() {
                                                        None => {}
                                                        Some(picture) => {
                                                            let data = picture.data();

                                                            fs::create_dir_all("~/.local/share/jams/covers/").expect("TODO: panic message");

                                                            let mut file = fs::OpenOptions::new()
                                                                .create(true) // To create a new file
                                                                .write(true)
                                                                // either use the ? operator or unwrap since it returns a Result
                                                                .open(path_to_write.clone()).unwrap();

                                                            file.write_all(&data).unwrap();
                                                        }
                                                    }

                                                    let new_album = Album {
                                                        album_artist: album_artist.clone(),
                                                        album: album.clone(),
                                                        cached_cover_path: path_to_write.clone(),
                                                        tracks: vec![index],
                                                    };
                                                    self.albums.push(new_album);
                                                }
                                            }

                                            self.scanned_files.push(music_file);
                                        } else {
                                            println!("No tags found in file");
                                            continue;
                                        };
                                    }
                                    Err(err) => eprintln!("Failed to run discovery: {err:?}"),
                                }
                            }
                        }
                        Err(entry_error) => {
                            println!("URL {} could not be read", url);
                        }
                    }
                }

                // https://rust-lang-nursery.github.io/rust-cookbook/algorithms/sorting.html#sort-a-vector-of-structs
                // Sorts a vec of structs by its natural order (aka the order that was declared in the struct)
                self.scanned_files.sort();
            }

            Message::StartPlayingNewTrack(uri) => {
                self.switch_track(uri);
            }

            Message::PauseCurrentTrack => {
                self.audio_player.player.pause();
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
                self.audio_player.player.play();
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

            Message::SearchExpand => {
                self.search_expanded = true;
            }

            Message::SearchMinimize => {
                self.search_term = "".to_string();
                self.search_expanded = false;
            }

            Message::SearchInput(term) => {
                self.search_term = term;
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
            Message::SaveLibraryLocation => {
                println!("This doesn't do anything right now.");
            }
            Message::ResetLibraryLocation => {
                println!("This doesn't do anything right now.");
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
        self.audio_player.player.stop();

        for file in &mut self.scanned_files {
            file.paused = false;
            if file.uri == uri {
                println!("Switching to track: {}", uri);
                file.playing = true;
                self.current_track_duration = file.duration;
            } else {
                file.playing = false;
            }
        }

        self.audio_player.player.set_uri(Some(uri.as_str()));

        self.audio_player.player.play();

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
        self.audio_player
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
