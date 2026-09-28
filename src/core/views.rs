use std::sync::Arc;
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, ContentFit, Length, Padding, Pixels, Radius};
use cosmic::{widget, Element, Theme};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{image, responsive, text, Column, Container, Grid};
use cosmic::widget::image::FilterMethod;
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

        let column_width = Length::Fixed((size.width / number_of_columns as f32) - 5.0);

        let mut grid: Grid<Message> = widget::Grid::new()
            .width(Length::Fill);

        let mut curr_column = 1;

        for album in &albums {
            let mut album_content = Column::new()
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .spacing(Pixels::from(5.0));

            let album_front_cover = image(album.cached_cover_path.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .border_radius(Radius::new(5.0))
                .filter_method(FilterMethod::Linear)
                .content_fit(ContentFit::Fill);

            let album_cover_container = Container::new(album_front_cover)
                .width(column_width)
                .height(column_width);

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

            let max_album_width = Length::Fixed((size.width / number_of_columns as f32) - 5.0); // 5.0 is to account for padding until scrollbar is improved.
            let album_container = Container::new(album_content)
                .width(max_album_width)
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