use std::path::Path;
use std::sync::Arc;
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, ContentFit, Length, Padding, Pixels, Radius};
use cosmic::{widget, Element, Theme};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{image, responsive, text, Column, Container, Grid, Image};
use cosmic::widget::image::{FilterMethod, Handle};
use crate::app::{Album, Message};

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

            let album_container = Container::new(album_content)
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