use std::collections::HashMap;
use std::iter::zip;
use std::ops::Range;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarState;
use ratatui::widgets::Widget;

type IndexInHeightLog = usize;
type PartialTopItem = Option<(IndexInHeightLog, Rect)>;
type FullItems = Option<(Range<IndexInHeightLog>, Rect)>;
type PartialBottomItem = Option<(IndexInHeightLog, Rect)>;

pub struct ScrollState {
    pub last_max_position: usize,
    pub position: usize,
    pub follow: bool,
    render_memo_cache: HashMap<u16, Vec<usize>>,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            last_max_position: 0,
            position: 0,
            follow: true,
            render_memo_cache: HashMap::new(),
        }
    }

    pub const fn scroll_down(&mut self) -> bool {
        let value_change = self.position < self.last_max_position;
        if value_change {
            self.position = self.position.saturating_add(1);
        }
        // When scrolled all the way to the bottom, auto follow the feed ("sticky behavior")
        self.follow = self.position == self.last_max_position;

        value_change
    }

    pub const fn scroll_up(&mut self) -> bool {
        self.follow = false;

        // Can't scroll up when already on top
        let value_change = self.position > 0;
        self.position = self.position.saturating_sub(1);

        value_change
    }

    fn get_render_memo_for_width(&mut self, width: u16, num_elements: usize) -> &Vec<usize> {
        let list = self.render_memo_cache.entry(width).or_insert_with(|| {
            let tmp_height_list = vec![1; num_elements];
            tmp_height_list
        });
        for _ in 0..num_elements - list.len() {
            list.push(1);
        }
        list
    }

    fn render_scrollbar(
        frame: &mut Frame,
        area: Rect,
        scroll_position: usize,
        max_scroll_offset: usize,
    ) {
        let scrollbar = Scrollbar::default()
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll_offset).position(max_scroll_offset - scroll_position);
        frame.render_stateful_widget(
            scrollbar,
            area.outer(Margin {
                vertical: 0,
                horizontal: 1,
            }),
            &mut scrollbar_state,
        );
    }

    pub fn render<'a, Element, ElementWidget, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        elements: &'a [Element],
        render_element: F,
    ) where
        ElementWidget: Widget,
        F: Fn(&'a Element) -> (usize, ElementWidget),
    {
        let viewport_height = area.height as usize;
        let viewport_width = area.width;

        let mut height_log = self
            .get_render_memo_for_width(viewport_width, elements.len())
            .clone();

        loop {
            let total_height: usize = height_log.iter().sum();
            let max_scroll_offset = total_height.saturating_sub(viewport_height);

            if self.follow {
                self.position = max_scroll_offset;
            }

            let scroll_offset = max_scroll_offset.saturating_sub(self.position);

            let result = try_render_scroll_state(
                frame,
                area,
                scroll_offset,
                &mut height_log,
                elements,
                &render_element,
            );

            if result.is_ok() {
                // Update the cache
                self.render_memo_cache.insert(viewport_width, height_log);

                self.last_max_position = max_scroll_offset;

                // More content than fits on the screen? Show scrollbar
                if total_height > viewport_height {
                    Self::render_scrollbar(frame, area, scroll_offset, max_scroll_offset);
                }
                break;
            }
        }
    }
}

fn try_render_scroll_state<'a, Element, ElementWidget, F>(
    frame: &mut Frame,
    draw_area: Rect,
    scroll_positon: usize,
    height_log: &mut [usize],
    elements: &'a [Element],
    render_element: F,
) -> Result<(), ()>
where
    ElementWidget: Widget,
    F: Fn(&'a Element) -> (usize, ElementWidget),
{
    let mut update_needed = false;
    let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
        get_areas_to_render_from_scroll_position(draw_area, scroll_positon, height_log);

    if let Some((index, area)) = partial_draw_top {
        let req = &elements[index];
        let (height, widget) = render_element(req);

        if height == height_log[index] {
            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            widget.render(buffer_area, &mut buffer);

            let visible_content = buffer
                .content
                .into_iter()
                .skip(area.width as usize * (height - area.height as usize))
                .take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        } else {
            height_log[index] = height;
            update_needed = true;
        }
    }

    if let Some((index, area)) = partial_draw_bottom {
        let req = &elements[index];
        let (height, widget) = render_element(req);

        if height == height_log[index] {
            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            widget.render(buffer_area, &mut buffer);

            let visible_content = buffer.content.into_iter().take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        } else {
            height_log[index] = height;
            update_needed = true;
        }
    }

    if let Some((range, area)) = full_draw_middle {
        let mut widget_blocks = vec![];
        let mut constrains = vec![];
        for index in range {
            let req = &elements[index];
            let (height, widget) = render_element(req);

            if height != height_log[index] {
                height_log[index] = height;
                update_needed = true;
            }

            if !update_needed {
                widget_blocks.push(widget);
                constrains.push(Constraint::Length(height.try_into().unwrap()));
            }
        }

        if !update_needed {
            let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
            for (a, widget) in zip(areas, widget_blocks) {
                widget.render(a, frame.buffer_mut());
            }
        }
    }

    if update_needed { Err(()) } else { Ok(()) }
}

/// Scrolling for arbitrary sized items.
///
/// Given an array of your items height, a scroll postion and the area where the items
/// will (later) be rendered into, returns which items will be shown and where they
/// need to be rendered.  
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
