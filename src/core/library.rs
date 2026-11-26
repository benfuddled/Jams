use infer::Infer;
use lofty::file::{TaggedFile, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use directories::ProjectDirs;
use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use walkdir::WalkDir;

// OLD
// impl Default for MusicFile {
//     fn default() -> Self {
//         MusicFile {
//             saved_path: PathBuf::new(),
//             uri: "/uri-does-not-exist".to_string(),
//             playing: false,
//             paused: false,
//             track_title: "Invalid Title".to_string(),
//             track_number: 0,
//             duration: Duration::new(0, 0),
//             artist: "Invalid Artist".to_string(),
//             album: "Invalid Album".to_string(),
//             album_artist: "Invalid Album Artist".to_string(),
//             date: "Invalid Date".to_string(),
//             id: 0,
//         }
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct AudioFile {
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) track_number: u16,
    pub(crate) id: Uuid,
    pub(crate) album_artist: String,
    pub(crate) track_title: String,
    pub(crate) duration: Duration,
    date: String,
    saved_path: PathBuf,
    pub(crate) uri: String,
    cover_art_path: Option<String>,
}

impl AudioFile {
    pub fn new(path: PathBuf, tagged_file: TaggedFile) -> Self {
        let url = Url::from_file_path(path.clone()).unwrap();
        let tags = tagged_file.primary_tag().unwrap();
        let track_title = match tags.get_string(&ItemKey::TrackTitle).map(|s| s.to_string()) {
            Some(title) => title,
            None => {
                // If there's no track tag, fall back to the file name.
                match path.file_name() {
                    Some(filename) => match filename.to_str() {
                        Some(filename) => filename.to_string(),
                        None => String::from(""),
                    },
                    None => String::from(""),
                }
            }
        };
        let album = tags
            .album()
            .map(|s| s.to_string())
            .unwrap_or_else(|| String::from("Unknown Album"));
        let artist = tags.artist().map(|s| s.to_string()).unwrap_or_default();
        let album_artist = match tags
            .get_string(&ItemKey::AlbumArtist)
            .map(|s| s.to_string())
        {
            Some(album_artist) => album_artist,
            None => artist.clone(),
        };
        let date = tags.year().map(|s| s.to_string()).unwrap_or_default();
        let track_number = match tags.track().map(|s| s.to_string()) {
            Some(track) => track.parse::<u16>().unwrap_or(0),
            None => 0,
        };

        let properties = lofty::prelude::AudioFile::properties(&tagged_file);
        let duration = Duration::from_secs(properties.duration().as_secs());

        let cover_art_path: Option<String> = match tags.pictures().first() {
            None => None,
            Some(picture) => {
                let data = picture.data();
                let file_name = format!("{}_{}_{}", artist.clone(), album.clone(), date.clone());
                let home_dir = std::env::var("HOME").unwrap();
                let path_to_write = format!("{}/.local/share/jams/covers/{}", home_dir, file_name);

                match Path::new(&path_to_write).try_exists() {
                    Ok(true) => {
                        println!(
                            "Cached album cover {} already exists, no write.",
                            path_to_write
                        )
                    }
                    Ok(false) => {
                        let dir = format!("{}/.local/share/jams/covers/", home_dir);
                        fs::create_dir_all(dir).expect("TODO: panic message");

                        match fs::OpenOptions::new()
                            .create(true) // To create a new file
                            .write(true)
                            // either use the ? operator or unwrap since it returns a Result
                            .open(path_to_write.clone())
                        {
                            Ok(mut file) => {
                                file.write_all(&data).unwrap();
                            }
                            Err(e) => {
                                println!("{}", e);
                            }
                        }
                    }
                    Err(e) => println!("couldn't determine: {e}"),
                }

                Option::from(path_to_write)
            }
        };

        AudioFile {
            album_artist: album_artist.clone(),
            album: album.clone(),
            track_number,
            artist,
            track_title,
            duration,
            date,
            saved_path: path,
            uri: url.to_string(),
            id: Uuid::new_v4(),
            cover_art_path,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Album {
    pub(crate) id: Uuid,
    pub(crate) album_artist: String,
    pub(crate) album: String,
    pub(crate) cover_art_path: Option<String>,
    pub(crate) tracks: Vec<Uuid>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Artist {
    id: Uuid,
    albums: Vec<Uuid>,
    tracks: Vec<Uuid>,
}

pub struct Player {
    currently_playing_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Library {
    pub(crate) files: Vec<AudioFile>,
    pub(crate) albums: Vec<Album>,
    pub(crate) artists: Vec<Artist>,
    pub(crate) current_track_id: Uuid,
}

impl Library {
    pub(crate) fn new() -> Self {
        Library {
            files: vec![],
            albums: vec![],
            artists: vec![],
            current_track_id: Uuid::default(),
        }
    }

    pub fn ingest_file(&mut self, audio_file: AudioFile) {
        match self.albums.iter_mut().find(|album| {
            album.album == audio_file.album && album.album_artist == audio_file.album_artist
        }) {
            Some(album) => {
                album.tracks.push(audio_file.id);
            }
            None => {
                let new_album = Album {
                    id: Uuid::new_v4(),
                    album_artist: audio_file.album_artist.clone(),
                    album: audio_file.album.clone(),
                    tracks: [audio_file.id].to_vec(),
                    cover_art_path: audio_file.cover_art_path.clone(),
                };
                self.albums.push(new_album);
            }
        }

        self.files.push(audio_file);
    }

    pub fn sort_files(&mut self) {
        self.files.sort();
    }
}

pub trait ArtistById {
    fn artist_by_id(&self, id: Uuid) -> Option<&Artist>;
}

impl ArtistById for Library {
    fn artist_by_id(&self, id: Uuid) -> Option<&Artist> {
        self.artists.iter().find(|artist| artist.id == id)
    }
}

pub trait AlbumById {
    fn album_by_id(&self, id: Uuid) -> Option<&Album>;
}

impl AlbumById for Library {
    fn album_by_id(&self, id: Uuid) -> Option<&Album> {
        self.albums.iter().find(|album| album.id == id)
    }
}

pub fn restore_library() -> Library {
    match ProjectDirs::from("com", "benfuddled",  "Jams") {
        Some(proj_dirs) => {
            let library_loc = proj_dirs.data_dir().join("library.ron");
            if library_loc.exists() {
                match fs::read_to_string(&library_loc) {
                    Ok(contents) => {
                        match ron::from_str(&contents) {
                            Ok(library) => {
                                library
                            },
                            Err(err_msg) => {
                                println!("Error loading library: {}", err_msg);
                                Library::new()
                            }
                        }
                    }
                    Err(err_msg) => {
                        println!("Error reading library file: {}", err_msg);
                        Library::new()
                    }
                }
            } else {
                Library::new()
            }
        }
        None => {
            println!("Could not find project directories");
            Library::new()
        }
    }
}

pub fn get_all_files(url: Url, library: &mut Library) {
    for (index, entry) in WalkDir::new(url.to_file_path().unwrap())
        .into_iter()
        .enumerate()
    {
        match entry {
            Ok(entry) => {
                let is_audio = is_audio_file(entry.path()).unwrap_or_else(|_| false);
                if entry.file_type().is_file() && is_audio {
                    println!("{}", entry.path().display());
                    match lofty::read_from_path(entry.clone().path()) {
                        Ok(file) => {
                            if let Some(tags) = file.primary_tag() {
                                let file = AudioFile::new(entry.clone().into_path(), file);
                                library.ingest_file(file);
                                library.sort_files();
                            } else {
                                println!("No tags found in file");
                            }
                        }
                        Err(err) => {
                            eprintln!("Error reading file: {}", err);
                        }
                    };
                }
            }
            Err(entry_error) => {
                println!("{}", entry_error);
            }
        }
    }

    // https://rust-lang-nursery.github.io/rust-cookbook/algorithms/sorting.html#sort-a-vector-of-structs
    // Sorts a vec of structs by its natural order (aka the order that was declared in the struct)
    library.files.sort();
}

pub fn cache_library(library: Library) {
    match to_string_pretty(&library, PrettyConfig::default()) {
        Ok(ron_string) => {
            if let Some(proj_dirs) = ProjectDirs::from("com", "benfuddled",  "Jams") {
                let file_loc = proj_dirs.data_dir().join("library.ron");
                match fs::write(file_loc, &ron_string) {
                    Ok(_) => {
                        println!("Wrote library.ron");
                    }
                    Err(err_msg) => {
                        println!("Error writing library.ron file: {}", err_msg);
                    }
                }
            }
        }
        Err(err_msg) => {
            println!("Error converting library to pretty string: {}", err_msg);
        }
    }
}

fn is_audio_file(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buf = [0; 1024]; // Read first KB for detection
    file.read_exact(&mut buf)?;

    let info = Infer::new();
    Ok(info.is_audio(&buf))
}
