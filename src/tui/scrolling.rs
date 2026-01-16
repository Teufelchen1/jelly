use ratatui::layout::Rect;

use std::ops::Range;

pub fn get_areas_to_render_from_scroll_position(
    area: Rect,
    mut scroll_offset: usize,
    height_log: &[usize],
) -> (
    Option<(usize, Rect)>,
    Option<(Range<usize>, Rect)>,
    Option<(usize, Rect)>,
) {
    // These are going to be our return values
    let mut area_for_partial_draw_top = None;
    let mut area_for_fully_drawn = None;
    let mut area_for_partial_draw_bottom = None;

    // The entire viewspace
    let viewable_space = usize::from(area.height);
    // Our iterator index, we iterate backwards
    let mut latest_item = height_log.len();

    // If we need to compute the area of a partially drawn Item on the top
    let mut has_partial_item_top: Option<usize> = None;
    // .. and/or on the bottom
    let mut has_partial_item_bottom: Option<usize> = None;

    let mut middle_space_available = viewable_space;
    let mut used_middle_space = 0;

    // Scroll backwards through the items until there are no items left
    // or we have no scroll offset left
    // or we still have scroll offset but its too little for the next item
    while scroll_offset > 0 && latest_item > 0 {
        latest_item -= 1;
        let latest_item_height = height_log[latest_item];
        if latest_item_height > scroll_offset {
            has_partial_item_bottom = Some(latest_item);
            break;
        }
        scroll_offset -= latest_item_height;
    }

    let stop_item_full_drawn = if let Some(index) = has_partial_item_bottom {
        let item_bottom_height = height_log[index];
        let remaining_item_bottom_height_after_scrolling = item_bottom_height - scroll_offset;
        let partial_item_bottom_height =
            if remaining_item_bottom_height_after_scrolling > viewable_space {
                middle_space_available = 0;
                viewable_space
            } else {
                middle_space_available -= remaining_item_bottom_height_after_scrolling;
                remaining_item_bottom_height_after_scrolling
            };
        area_for_partial_draw_bottom = Some((
            index,
            Rect {
                y: area.y
                    + middle_space_available
                        .try_into()
                        .unwrap_or(u16::max_value() - area.y),
                height: partial_item_bottom_height
                    .try_into()
                    .unwrap_or(u16::max_value()),
                ..area
            },
        ));
        index
    } else {
        latest_item
    };

    while latest_item > 0 && middle_space_available > 0 {
        latest_item -= 1;
        let latest_item_height = height_log[latest_item];
        if latest_item_height > middle_space_available {
            has_partial_item_top = Some(latest_item);
            latest_item += 1;
            break;
        }
        middle_space_available -= latest_item_height;
        used_middle_space += latest_item_height;
    }

    let start_item_full_draw = latest_item;

    let remaining_space_top = if let Some(index) = has_partial_item_top {
        let remaining_space_top = middle_space_available
            .try_into()
            .unwrap_or(u16::max_value());
        area_for_partial_draw_top = Some((
            index,
            Rect {
                height: remaining_space_top,
                ..area
            },
        ));
        remaining_space_top
    } else {
        0
    };

    if used_middle_space > 0 {
        area_for_fully_drawn = Some((
            start_item_full_draw..stop_item_full_drawn,
            Rect {
                y: area.y + remaining_space_top,
                height: used_middle_space.try_into().unwrap_or(u16::max_value()),
                ..area
            },
        ))
    }

    return (
        area_for_partial_draw_top,
        area_for_fully_drawn,
        area_for_partial_draw_bottom,
    );
    // let mut counter_offset = height_log.len();
    // while scroll_offset > 0 && counter_offset > 0 {
    //     counter_offset -= 1;
    //     scroll_offset = if let Some(sub) = scroll_offset.checked_sub(height_log[counter_offset]) {
    //         sub
    //     } else {
    //         break;
    //     };
    // }

    // let mut inner_height = usize::from(area.height) - scroll_offset;
    // let mut used_inner_height = 0;
    // let mut counter_start = counter_offset;
    // while inner_height > 0 && counter_start > 0 {
    //     counter_start -= 1;
    //     inner_height = if let Some(sub) = inner_height.checked_sub(height_log[counter_start]) {
    //         used_inner_height += height_log[counter_start];
    //         sub
    //     } else {
    //         counter_start += 1;
    //         break;
    //     }
    // }

    // let mut remainder_top = 0u16;
    // if counter_start > 0 && inner_height > 0 {
    //     remainder_top = inner_height.try_into().unwrap_or(u16::max_value());
    // }

    // let area_for_partial_draw_top = Rect {
    //     height: scroll_offset.try_into().unwrap_or(u16::max_value()) + remainder_top,
    //     ..area
    // };

    // let area_for_fully_drawn = Rect {
    //     y: area.y + area_for_partial_draw_top.height,
    //     height: used_inner_height.try_into().unwrap_or(u16::max_value()),
    //     ..area
    // };

    // let area_for_partial_draw_bottom = Rect {
    //     y: area_for_fully_drawn.y + area_for_fully_drawn.height,
    //     height: inner_height as u16,
    //     ..area
    // };

    // println!("{counter_start}|{scroll_offset}");
    // let partial_draw_top = if counter_start > 0 && (scroll_offset > 0 || inner_height > 0) {
    //     Some((counter_start - 1, area_for_partial_draw_top))
    // } else {
    //     None
    // };

    // let full_draw_middle = (counter_start..counter_offset, area_for_fully_drawn);

    // let partial_draw_bottom = if counter_offset < height_log.len() && scroll_offset > 0 {
    //     Some((counter_offset, area_for_partial_draw_bottom))
    // } else {
    //     None
    // };

    //return (partial_draw_top, full_draw_middle, partial_draw_bottom);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_item_fits_with_remaining_space() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [4];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 0..1);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 4,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn single_item_fits_perfect() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [10];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 0..1);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 10,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn two_item_fits_with_remaining_space() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [4, 5];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 0..2);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 4 + 5,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn two_item_fits_perfect() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [4, 6];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 0..2);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 4 + 6,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn three_item_no_fit() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [4, 5, 3];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_some());
        let (index, area) = partial_draw_top.unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 2,
                width: 5,
            }
        );

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 1..3);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 2,
                height: 5 + 3,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn three_item_no_fit_scrolled() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 1;
        let height_log = [4, 5, 3];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_some());
        let (index, area) = partial_draw_top.unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 3,
                width: 5,
            }
        );

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 1..2);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 3,
                height: 5,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_some());
        let (index, area) = partial_draw_bottom.unwrap();
        assert_eq!(index, 2);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 8,
                height: 2,
                width: 5,
            }
        );
    }

    #[test]
    fn two_item_perfect_fit_scrolled() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 3;
        let height_log = [4, 6, 3];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 0..2);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 4 + 6,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn three_item_perfect_fit_scrolled() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 30;
        // ....................|.......|..<< 30...
        let height_log = [4, 6, 3, 6, 1, 20, 9, 1];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 2..5);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 3 + 6 + 1,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn three_item_no_fit_scrolled_far() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 33;
        // ..................|.....|.....<< 33...
        let height_log = [4, 6, 3, 6, 1, 20, 9, 1];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_some());
        let (index, area) = partial_draw_top.unwrap();
        assert_eq!(index, 1);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 3,
                width: 5,
            }
        );

        let (range, area) = full_draw_middle.unwrap();
        assert_eq!(range, 2..3);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 3,
                height: 3,
                width: 5,
            }
        );

        assert!(partial_draw_bottom.is_some());
        let (index, area) = partial_draw_bottom.unwrap();
        assert_eq!(index, 3);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 6,
                height: 4,
                width: 5,
            }
        );
    }
}
