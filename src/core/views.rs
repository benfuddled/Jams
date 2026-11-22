use std::path::Path;
use std::sync::Arc;
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, Color, ContentFit, Length, Padding, Pixels, Radius, Vector};
use cosmic::{widget, Element, Theme};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::theme::Button;
use cosmic::widget::{button, icon, image, responsive, scrollable, text, Column, Container, Grid, Image, Row};
use cosmic::widget::button::Style;
use cosmic::widget::image::{FilterMethod, Handle};
use crate::app::{Album, Message, MusicFile};

pub fn album_grid<'a>(albums: Vec<Album>) -> Element<'a, Message> {
    let responsive_grid = responsive(move |size| {
        let mut number_of_columns = 1;
        let mut gap = 0.0;

        // This could be set by a slider.
        if size.width > 1600.0 {
            number_of_columns = 6;
            gap = 12.0;
        } else if size.width > 1200.0 {
            number_of_columns = 5;
            gap = 10.0;
        } else if size.width > 1000.0 {
            number_of_columns = 4;
            gap = 8.0;
        } else if size.width > 650.0 {
            number_of_columns = 3;
            gap = 6.0;
        } else if size.width > 350.0 {
            number_of_columns = 2;
            gap = 4.0;
        }

        // libcosmic scrollbars tend to overlap content, let's leave some space at the end.
        let scrollbar_overlay_fix = (18.0 / number_of_columns as f32).ceil();

        let column_width = (size.width.floor() / number_of_columns as f32).floor() - scrollbar_overlay_fix;
        let album_size = column_width - (gap * 2.0);

        let mut grid: Grid<Message> = widget::Grid::new()
            .width(Length::Fill);

        let mut curr_column = 1;

        for album in &albums {

            let mut album_content = Column::new()
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .spacing(Pixels::from(5.0));

            let album_front_cover: Image = match &album.cached_cover_path {
                None => {
                    let blank_album_image_handle: Handle = match cosmic::theme::is_dark() {
                        true => {
                            image::Handle::from_path(Path::new("./res/graphics/album-dark-mode.png"))
                        }
                        false => {
                            image::Handle::from_path(Path::new("./res/graphics/album-light-mode.png"))
                        }
                    };

                    image(blank_album_image_handle)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .border_radius(Radius::new(5.0))
                        .filter_method(FilterMethod::Linear)
                        .content_fit(ContentFit::Fill)
                }
                Some(path) => {
                    image(path.clone())
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .border_radius(Radius::new(5.0))
                        .filter_method(FilterMethod::Linear)
                        .content_fit(ContentFit::Fill)
                }
            };

            let album_cover_container = Container::new(album_front_cover)
                .width(Length::Fixed(album_size))
                .height(Length::Fixed(album_size));

            let mut album_text = Column::new()
                .align_x(Alignment::Center)
                .height(Length::Fixed(45.0))
                .width(Length::Fill);

            let album_name = text::caption_heading(album.album.clone())
                .width(Length::Fill)
                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                .align_y(Alignment::Center)
                .align_x(Alignment::Center);

            let album_artist = text::caption(album.album_artist.clone())
                .width(Length::Fill)
                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                .align_y(Alignment::Center)
                .align_x(Alignment::Center);

            album_text = album_text
                .push(album_name)
                .push(album_artist);

            album_content = album_content
                .push(album_cover_container)
                .push(album_text);

            let album_button = button::custom_image_button(album_content, None)
                .on_press(Message::SelectAlbum(album.album_id))
                .padding(0)
                .class(Button::Custom {
                    active: Box::new(|_, _| Style {
                        shadow_offset: Vector::new(0.0, 0.0),
                        background: None,
                        overlay: None,
                        border_radius: Radius::from(0.0),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                        outline_width: 0.0,
                        outline_color: Color::TRANSPARENT,
                        icon_color: None,
                        text_color: None,
                    }),
                    disabled: Box::new(|_| Style {
                        shadow_offset: Vector::new(0.0, 0.0),
                        background: None,
                        overlay: None,
                        border_radius: Radius::from(0.0),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                        outline_width: 0.0,
                        outline_color: Color::TRANSPARENT,
                        icon_color: None,
                        text_color: None,
                    }),
                    hovered: Box::new(|_, _| Style {
                        shadow_offset: Vector::new(0.0, 0.0),
                        background: None,
                        overlay: None,
                        border_radius: Radius::from(0.0),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                        outline_width: 0.0,
                        outline_color: Color::TRANSPARENT,
                        icon_color: None,
                        text_color: None,
                    }),
                    pressed: Box::new(|_, _| Style {
                        shadow_offset: Vector::new(0.0, 0.0),
                        background: None,
                        overlay: None,
                        border_radius: Radius::from(0.0),
                        border_width: 0.0,
                        border_color: Color::TRANSPARENT,
                        outline_width: 0.0,
                        outline_color: Color::TRANSPARENT,
                        icon_color: None,
                        text_color: None,
                    }),
                });

            let album_container = Container::new(album_button)
                .width(Length::Fixed(column_width))
                .padding(Padding::from([0.0, gap]))
                .align_x(Alignment::Center);

            grid = grid.push(album_container);

            if curr_column % number_of_columns == 0 {
                grid = grid.insert_row();
            }

            curr_column = curr_column + 1;
        }
        grid.into()
    });
    Container::new(responsive_grid).into()
}

pub fn album_tracklist<'a>(list: &Vec<MusicFile>, album: &Album) -> Element<'a, Message> {
    let mut top_bar = Row::new()
        .spacing(Pixels::from(20))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let album_front_cover: Image = match &album.cached_cover_path {
        None => {
            let blank_album_image_handle: Handle = match cosmic::theme::is_dark() {
                true => {
                    image::Handle::from_path(Path::new("./res/graphics/album-dark-mode.png"))
                }
                false => {
                    image::Handle::from_path(Path::new("./res/graphics/album-light-mode.png"))
                }
            };

            image(blank_album_image_handle)
                .width(Length::Fill)
                .height(Length::Fill)
                .border_radius(Radius::new(5.0))
                .filter_method(FilterMethod::Linear)
                .content_fit(ContentFit::Fill)
        }
        Some(path) => {
            image(path.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .border_radius(Radius::new(5.0))
                .filter_method(FilterMethod::Linear)
                .content_fit(ContentFit::Fill)
        }
    };

    let album_cover_container = Container::new(album_front_cover)
        .width(Length::Fixed(180.0))
        .height(Length::Fixed(180.0));

    let name = text::title2(album.album.clone())
        .width(Length::Fill)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .align_y(Alignment::Center)
        .align_x(Alignment::Start);

    let artist = text::title3(album.album_artist.clone())
        .width(Length::Fill)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
        .align_y(Alignment::Center)
        .align_x(Alignment::Start);

    let text_col = Column::new().push(name).push(artist);

    let tracklist = tracklist(&album.tracks, &String::from(""));

    top_bar = top_bar.push(album_cover_container).push(text_col);

    let mut content = Column::new().align_x(Alignment::Center);
    content = content.push(top_bar);
    content = content.push(tracklist);

    Container::new(content).into()
}

pub fn tracklist<'a>(list: &Vec<MusicFile>, filter: &String) -> Element<'a, Message> {
    let mut file_col = Column::new().spacing(2);

    for file in list {
        if filter.is_empty()
            || file
            .album
            .to_lowercase()
            .contains(&filter.to_lowercase())
            || file
            .artist
            .to_lowercase()
            .contains(&filter.to_lowercase())
            || file
            .track_title
            .to_lowercase()
            .contains(&filter.to_lowercase())
            || file
            .album_artist
            .to_lowercase()
            .contains(&filter.to_lowercase())
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
                let button =
                    button::icon(icon::from_name("media-playback-start-symbolic"))
                        .on_press(Message::ResumeCurrentTrack)
                        .tooltip("Resume");
                file_txt_row = file_txt_row.push(button);
            } else if file.playing == true {
                let button =
                    button::icon(icon::from_name("media-playback-pause-symbolic"))
                        .on_press(Message::PauseCurrentTrack)
                        .tooltip("Pause");
                file_txt_row = file_txt_row.push(button);
            } else {
                let button =
                    button::icon(icon::from_name("media-playback-start-symbolic"))
                        .on_press(Message::StartPlayingNewTrack(file.uri.clone()))
                        .tooltip("Play");
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
        }
    }

    Container::new(file_col).into()
}