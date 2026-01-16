use ratatui::layout::Rect;

use std::ops::Range;

type IndexInHeightLog = usize;
type PartialTopItem = Option<(IndexInHeightLog, Rect)>;
type FullItems = Option<(Range<IndexInHeightLog>, Rect)>;
type PartialBottomItem = Option<(IndexInHeightLog, Rect)>;

pub fn get_areas_to_render_from_scroll_position(
    area: Rect,
    mut scroll_offset: usize,
    height_log: &[usize],
) -> (PartialTopItem, FullItems, PartialBottomItem) {
    // These are going to be our return values
    let mut area_for_partial_draw_top = None;
    let mut area_for_fully_drawn = None;
    let mut area_for_partial_draw_bottom = None;

    // The entire viewspace
    let viewable_space = usize::from(area.height);
    // Our iterator index, we iterate backwards
    let mut latest_item = height_log.len();

    // If we need to compute the area of a partially drawn Item on the top
    let mut has_partial_item_top: Option<IndexInHeightLog> = None;
    // .. and/or on the bottom
    let mut has_partial_item_bottom: Option<IndexInHeightLog> = None;

    let mut middle_space_available = viewable_space;
    let mut used_middle_space = 0;

    // Scroll backwards through the items until:
    // there are no items left
    // or we have no scroll offset left
    // or we still have scroll offset left but its too little for the next item
    while scroll_offset > 0 && latest_item > 0 {
        latest_item -= 1;
        let latest_item_height = height_log[latest_item];
        // Do we have enough scroll_offset left to scroll past the current item?
        if latest_item_height > scroll_offset {
            // No, we don't.
            // So the current item will be drawn at the bottom
            // (At the bottom because it is the most recent item)
            has_partial_item_bottom = Some(latest_item);
            break;
        }
        // Yes we can completly scroll past this item
        scroll_offset -= latest_item_height;
    }

    // At which item do we need to stop drawing?
    // When we have a partial item at the bottom, that item is also the stop item.
    // If we don't have a partial item at the bottom,
    // the stop item is the lastest item, it's just outside our viewable space.
    // e.g. if we have a scroll_offset of 0 (no scrolling going on), the stop item
    // will be its [latest_item] initial value of height_log.len().
    let stop_item_full_drawn = latest_item;

    if let Some(index) = has_partial_item_bottom {
        // Calculate the amount of space of the partial item that is still
        // inside the viewable area.
        let remaining_item_bottom_height_after_scrolling = height_log[index] - scroll_offset;
        let partial_item_bottom_height =
            // There is one edge case, where the item is so big, it overflows the
            // viewable area. In that case, there is no space left for anything else,
            // and the item is limited to the viewable space.
            if remaining_item_bottom_height_after_scrolling > viewable_space {
                middle_space_available = 0;
                viewable_space
            } else {
                // Calculate how much space remains for other items
                middle_space_available -= remaining_item_bottom_height_after_scrolling;
                remaining_item_bottom_height_after_scrolling
            };

        // Store the return value
        area_for_partial_draw_bottom = Some((
            index,
            Rect {
                // We add the remaining space as we start drawing from the bottom up
                y: area.y
                    + middle_space_available
                        .try_into()
                        .unwrap_or(u16::MAX - area.y),
                height: partial_item_bottom_height.try_into().unwrap_or(u16::MAX),
                ..area
            },
        ));
    }

    // Calculate how many items fit inside the remaining available space in the middle.
    // (in the middle between Option<partially top> and Option<partially bottom>)
    // We do that until:
    // There are not items left
    // or there is no space left to fit the next item
    while latest_item > 0 && middle_space_available > 0 {
        latest_item -= 1;
        let latest_item_height = height_log[latest_item];
        // Does the item fit into the remaining space?
        if latest_item_height > middle_space_available {
            // No it does not.
            // This means we need to have a partially drawn item at the top
            has_partial_item_top = Some(latest_item);

            // This is going to be the start item for the fully drawn items.
            // Since the current item is already only partially drawn, we offset by 1.
            latest_item += 1;
            break;
        }
        middle_space_available -= latest_item_height;

        // Track how much space we covered with fully drawn items.
        used_middle_space += latest_item_height;
    }

    // Just a rename for clarity. This tells us where with which item we should start
    // drawing the fully drawn items.
    let start_item_full_draw = latest_item;

    // How much space is covered by the top item that is only partially drawn (if any)?
    let remaining_space_top = if let Some(index) = has_partial_item_top {
        let remaining_space_top = middle_space_available.try_into().unwrap_or(u16::MAX);
        area_for_partial_draw_top = Some((
            index,
            Rect {
                height: remaining_space_top,
                ..area
            },
        ));
        remaining_space_top
    } else {
        // If we don't have a partial top item, it covers no space at all
        0
    };

    // Only draw full items...if we actually managed to fit any.
    if used_middle_space > 0 {
        area_for_fully_drawn = Some((
            start_item_full_draw..stop_item_full_drawn,
            Rect {
                // Offset to not overlap with the partially drawn top item (if any)
                y: area.y + remaining_space_top,
                height: used_middle_space.try_into().unwrap_or(u16::MAX),
                ..area
            },
        ));
    }

    (
        area_for_partial_draw_top,
        area_for_fully_drawn,
        area_for_partial_draw_bottom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_item_no_fit_no_scroll() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 0;
        let height_log = [14];

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
                height: 10,
                width: 5,
            }
        );
        assert!(full_draw_middle.is_none());
        assert!(partial_draw_bottom.is_none());
    }

    #[test]
    fn single_item_no_fit_with_partial_scroll() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 2;
        let height_log = [14];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());
        assert!(full_draw_middle.is_none());
        assert!(partial_draw_bottom.is_some());
        let (index, area) = partial_draw_bottom.unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 10,
                width: 5,
            }
        );
    }

    #[test]
    fn single_item_no_fit_full_scroll() {
        let area = Rect {
            x: 0,
            y: 0,
            height: 10,
            width: 5,
        };
        let scroll_offset = 4;
        let height_log = [14];

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

        assert!(partial_draw_top.is_none());
        assert!(full_draw_middle.is_none());
        assert!(partial_draw_bottom.is_some());
        let (index, area) = partial_draw_bottom.unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            area,
            Rect {
                x: 0,
                y: 0,
                height: 10,
                width: 5,
            }
        );
    }

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
