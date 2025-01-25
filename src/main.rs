// SPDX-License-Identifier: GPL-3.0-only


// Justification: Fields on DecoderOptions and FormatOptions may change at any time, but
// symphonia-play doesn't want to be updated every time those fields change, therefore always fill
// in the remaining fields with default values.
#![allow(clippy::needless_update)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use lazy_static::lazy_static;
use symphonia::core::codecs::{DecoderOptions, FinalizeResult, CODEC_TYPE_NULL};
use symphonia::core::errors::{Error, Result};
use symphonia::core::formats::{Cue, FormatOptions, FormatReader, SeekMode, SeekTo, Track};
use symphonia::core::io::{MediaSource, MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::{ColorMode, MetadataOptions, MetadataRevision, Tag, Value, Visual};
use symphonia::core::probe::{Hint, ProbeResult};
use symphonia::core::units::{Time, TimeBase};

use clap::{Arg, ArgMatches};
use log::{error, info, warn};

use app::Yamp;

/// The `app` module is used by convention to indicate the main component of our application.
mod app;
mod core;
mod player;
mod output;
mod resampler;
mod icon_cache;

/// The `cosmic::app::run()` function is the starting point of your application.
/// It takes two arguments:
/// - `settings` is a structure that contains everything relevant with your app's configuration, such as antialiasing, themes, icons, etc...
/// - `()` is the flags that your app needs to use before it starts.
///  If your app does not need any flags, you can pass in `()`.
fn main() -> cosmic::iced::Result {
    // For any error, return an exit code -1. Otherwise return the exit code provided.
    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<Yamp>(settings, ())
}
