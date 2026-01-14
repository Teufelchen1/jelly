use ratatui::layout::{Position, Rect};

/// State of a [`Table`] widget
///
/// This state can be used to scroll through the rows and select one of them. When the table is
/// rendered as a stateful widget, the selected row, column and cell will be highlighted and the
/// table will be shifted to ensure that the selected row is visible. This will modify the
/// [`State`] object passed to the `Frame::render_stateful_widget` method.
///
/// # Example
///
/// ```rust
/// use ratatui::Frame;
/// use ratatui::layout::{Constraint, Rect};
/// use ratatui::text::Line;
/// use ratatui_logline_table::{State, Table};
///
/// # fn ui(frame: &mut Frame) {
/// # let area = Rect::ZERO;
/// let rows = [["Cell1", "Cell2"], ["Cell3", "Cell4"]];
/// let widths = [Constraint::Length(5), Constraint::Length(5)];
/// let table = Table::new(&rows, widths, |_index, line| {
///     [Line::raw(line[0]), Line::raw(line[1])]
/// });
///
/// // Note: State should be stored in your application state (not constructed in your render
/// // method) so that the selected row is preserved across renders
/// let mut state = State::default();
/// *state.offset_mut() = 1; // display the second row and onwards
/// state.select(Some(3)); // select the forth row (0-indexed)
///
/// frame.render_stateful_widget(table, area, &mut state);
/// # }
/// ```
///
/// [`Table`]: super::Table
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[must_use]
pub struct State {
    pub(crate) offset: usize,
    pub(crate) selected: Option<usize>,
    pub(crate) ensure_selected_in_view_on_next_render: bool,
    pub(crate) scroll_keeps_last_in_view: bool,
    pub(crate) last_row_area: Rect,
    pub(crate) last_biggest_index: usize,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Creates a new [`State`]
    pub const fn new() -> Self {
        Self {
            offset: 0,
            selected: None,
            ensure_selected_in_view_on_next_render: false,
            scroll_keeps_last_in_view: true,
            last_row_area: Rect::ZERO,
            last_biggest_index: 0,
        }
    }

    /// Sets the index of the first row to be displayed
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Index of the first row to be displayed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_logline_table::State;
    /// let state = State::new();
    /// assert_eq!(state.offset(), 0);
    /// ```
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Mutable reference to the index of the first row to be displayed
    pub const fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Index of the selected row
    ///
    /// Returns `None` if no row is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui_logline_table::State;
    ///
    /// let state = State::new();
    /// assert_eq!(state.selected(), None);
    /// ```
    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Sets the index of the selected row
    ///
    /// Set to `None` if no row is selected.
    pub fn select(&mut self, index: Option<usize>) -> bool {
        self.ensure_selected_in_view_on_next_render = true;
        let mut changed = self.selected != index;
        self.selected = index;
        if index.is_none() && !self.scroll_keeps_last_in_view {
            self.scroll_keeps_last_in_view = true;
            changed = true;
        }
        changed
    }

    /// Selects the next row or the first one if no row is selected
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `0` and will be corrected when the table is rendered
    pub fn select_next(&mut self) -> bool {
        let next = self.selected.map_or(0, |i| i.saturating_add(1));
        self.select(Some(next))
    }

    /// Selects the previous row or the last one if no item is selected
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the table is rendered
    pub fn select_previous(&mut self) -> bool {
        let previous = self.selected.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select(Some(previous))
    }

    /// Selects the first row
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `0` and will be corrected when the table is rendered
    pub fn select_first(&mut self) -> bool {
        self.select(Some(0))
    }

    /// Selects the last row
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the table is rendered
    pub fn select_last(&mut self) -> bool {
        self.select(Some(usize::MAX))
    }

    /// Scrolls down by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it down by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., if the index is greater than
    /// the number of rows in the table), the last row in the table will be selected.
    pub fn scroll_down_by(&mut self, amount: usize) -> bool {
        let before = self.offset;
        self.offset = self
            .offset
            .saturating_add(amount)
            .min(self.last_biggest_index);
        before != self.offset
    }

    /// Scrolls up by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it up by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., less than zero),
    /// the first row in the table will be selected.
    pub const fn scroll_up_by(&mut self, amount: usize) -> bool {
        self.scroll_keeps_last_in_view = false;
        let before = self.offset;
        self.offset = self.offset.saturating_sub(amount);
        before != self.offset
    }

    /// Get the index of the logline on the given render position.
    ///
    /// This is useful for mouse interactions like [`select_at`](Self::select_at) on click.
    #[must_use]
    pub const fn rendered_at(&self, position: Position) -> Option<usize> {
        if !self.last_row_area.contains(position) {
            return None;
        }

        let row_in_view = position.y.saturating_sub(self.last_row_area.top());
        let index = self.offset.saturating_add(row_in_view as usize);
        Some(index)
    }

    /// Select the index of the logline on the given render position.
    ///
    /// Returns `true` when the state changed.
    /// Returns `false` when there was nothing at the given position.
    pub fn select_at(&mut self, position: Position) -> bool {
        self.rendered_at(position)
            .is_some_and(|index| self.select(Some(index)))
    }
}
