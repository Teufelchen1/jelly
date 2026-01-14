//! The [`Table`] widget is used to display multiple rows and columns in a grid and allows selecting
//! one or multiple cells.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Style, Styled};
use ratatui::text::{Line, Text};
pub use ratatui::widgets::{Block, BlockExt as _};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::widgets::{StatefulWidget, Widget};

use super::logline_highlight::HighlightSpacing;
use super::logline_state::State;

/// A widget to display data in formatted columns.
///
/// A `Table` is a collection of rows, each composed of cells:
///
/// You can construct a [`Table`] using [`Table::new`] and then chain
/// builder style methods to set the desired properties.
///
/// Table cells can be aligned, for more details see [`Line`].
///
/// [`Table`] implements [`Widget`] and so it can be drawn using `Frame::render_widget`.
///
/// [`Table`] is also a [`StatefulWidget`], which means you can use it with [`State`] to allow
/// the user to scroll through the rows and select one of them. When rendering a [`Table`] with a
/// [`State`], the selected row, column and cell will be highlighted. If the selected row is
/// not visible (based on the offset), the table will be scrolled to make the selected row visible.
///
/// Note: Highlight styles are applied in the following order: Row, Column, Cell.
///
/// # Example
///
/// ```rust
/// use ratatui::layout::Constraint;
/// use ratatui::style::{Style, Stylize};
/// use ratatui::text::Line;
/// use ratatui::widgets::Block;
/// use ratatui_logline_table::Table;
///
/// let rows = [["Cell1", "Cell2", "Cell3"]];
/// // Columns widths are constrained in the same way as Layout...
/// let widths = [
///     Constraint::Length(5),
///     Constraint::Length(5),
///     Constraint::Length(10),
/// ];
/// let table = Table::new(&rows, widths, |_index, line| {
///     [Line::raw(line[0]), Line::raw(line[1]), Line::raw(line[2])]
/// })
/// // ...and they can be separated by a fixed spacing.
/// .column_spacing(1)
/// // You can set the style of the entire Table.
/// .style(Style::new().blue())
/// // It has an optional header, which is simply a Row always visible at the top.
/// .header([Line::raw("Col1"), Line::raw("Col2"), Line::raw("Col3")])
/// .header_style(Style::new().bold())
/// // As any other widget, a Table can be wrapped in a Block.
/// .block(Block::new().title("Table"))
/// // The selected row and its content can also be styled.
/// .row_highlight_style(Style::new().reversed())
/// // ...and potentially show a symbol in front of the selection.
/// .highlight_symbol(">>");
/// ```
///
/// `Table` also implements the [`Styled`] trait, which means you can use style shorthands from
/// the [`Stylize`] trait to set the style of the widget more concisely.
///
/// ```rust
/// use ratatui::style::Stylize;
/// # use ratatui::{layout::Constraint, text::Line};
/// # use ratatui_logline_table::Table;
/// # let rows = [["Cell1", "Cell2"]];
/// # let widths = [Constraint::Length(5), Constraint::Length(5)];
/// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
/// let table = Table::new(&rows, widths, to_row).red().italic();
/// ```
///
/// # Stateful example
///
/// `Table` is a [`StatefulWidget`], which means you can use it with [`State`] to allow the
/// user to scroll through the rows and select one of them.
///
/// ```rust
/// use ratatui::Frame;
/// use ratatui::layout::{Constraint, Rect};
/// use ratatui::style::{Style, Stylize};
/// use ratatui::text::Line;
/// use ratatui::widgets::Block;
/// use ratatui_logline_table::{State, Table};
///
/// # fn ui(frame: &mut Frame) {
/// # let area = Rect::ZERO;
/// // Note: State should be stored in your application state (not constructed in your render
/// // method) so that the selected row is preserved across renders
/// let mut state = State::default();
/// let rows = [
///     ["Row11", "Row12", "Row13"],
///     ["Row21", "Row22", "Row23"],
///     ["Row31", "Row32", "Row33"],
/// ];
/// let widths = [
///     Constraint::Length(5),
///     Constraint::Length(5),
///     Constraint::Length(10),
/// ];
/// let table = Table::new(&rows, widths, |_index, line| {
///     [Line::raw(line[0]), Line::raw(line[1]), Line::raw(line[2])]
/// })
/// .block(Block::new().title("Table"))
/// .row_highlight_style(Style::new().reversed())
/// .highlight_symbol(">>");
///
/// frame.render_stateful_widget(table, area, &mut state);
/// # }
/// ```
///
/// [`Stylize`]: ratatui::style::Stylize
#[must_use]
pub struct Table<'a, const COLUMNS: usize, Logline> {
    lines: &'a [Logline],

    to_row: Box<dyn Fn(usize, &'a Logline) -> [Line<'a>; COLUMNS]>,

    /// Optional header
    header: Option<[Line<'a>; COLUMNS]>,

    /// Optional footer
    footer: Option<[Line<'a>; COLUMNS]>,

    /// Width constraints for each column
    widths: [Constraint; COLUMNS],

    /// Space between each column
    column_spacing: u16,

    /// A block to wrap the widget in
    block: Option<Block<'a>>,

    /// Base style for the widget
    style: Style,

    header_style: Style,

    footer_style: Style,

    /// Style used to render the selected row
    row_highlight_style: Style,

    /// Symbol in front of the selected row
    highlight_symbol: Text<'a>,

    /// Decides when to allocate spacing for the row selection
    highlight_spacing: HighlightSpacing,

    /// Controls how to distribute extra space among the columns
    flex: Flex,
}

impl<'a, const COLUMNS: usize, Logline> Table<'a, COLUMNS, Logline> {
    /// Creates a new [`Table`] widget with the given rows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::layout::Constraint;
    /// use ratatui::text::Line;
    /// use ratatui_logline_table::Table;
    ///
    /// let rows = [["Cell1", "Cell2"], ["Cell3", "Cell4"]];
    /// let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// let table = Table::new(&rows, widths, |_index, line| {
    ///     [Line::raw(line[0]), Line::raw(line[1])]
    /// });
    /// ```
    pub fn new<ToRowFn>(
        lines: &'a [Logline],
        widths: [Constraint; COLUMNS],
        to_row: ToRowFn,
    ) -> Self
    where
        ToRowFn: Fn(usize, &'a Logline) -> [Line<'a>; COLUMNS] + 'static,
    {
        ensure_percentages_less_than_100(&widths);
        Self {
            lines,
            to_row: Box::new(to_row),
            widths,
            header: None,
            footer: None,
            column_spacing: 1,
            block: None,
            style: Style::new(),
            header_style: Style::new(),
            footer_style: Style::new(),
            row_highlight_style: Style::new(),
            highlight_symbol: Text::default(),
            highlight_spacing: HighlightSpacing::default(),
            flex: Flex::Start,
        }
    }

    /// Sets the header row which will be displayed at the top of the [`Table`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::{layout::Constraint, text::Line};
    /// # use ratatui_logline_table::Table;
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let header = [Line::raw("Header Cell 1"), Line::raw("Header Cell 2")];
    /// let table = Table::new(&rows, widths, to_row).header(header);
    /// ```
    pub fn header(mut self, header: [Line<'a>; COLUMNS]) -> Self {
        self.header = Some(header);
        self
    }

    /// Sets the footer row which will be displayed at the bottom of the [`Table`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::{layout::Constraint, text::Line};
    /// # use ratatui_logline_table::Table;
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let footer = [Line::raw("Footer Cell 1"), Line::raw("Footer Cell 2")];
    /// let table = Table::new(&rows, widths, to_row).footer(footer);
    /// ```
    pub fn footer(mut self, footer: [Line<'a>; COLUMNS]) -> Self {
        self.footer = Some(footer);
        self
    }

    pub const fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    pub const fn footer_style(mut self, style: Style) -> Self {
        self.footer_style = style;
        self
    }

    /// Set the spacing between columns
    pub const fn column_spacing(mut self, spacing: u16) -> Self {
        self.column_spacing = spacing;
        self
    }

    /// Wraps the table with a custom [`Block`] widget.
    ///
    /// The `block` parameter is of type [`Block`]. This holds the specified block to be
    /// created around the [`Table`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::Block;
    /// # use ratatui::{layout::Constraint, text::Line};
    /// # use ratatui_logline_table::Table;
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let block = Block::bordered().title("Table");
    /// let table = Table::new(&rows, widths, to_row).block(block);
    /// ```
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets the base style of the widget
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// All text rendered by the widget will use this style, unless overridden by [`Block::style`],
    /// or the styles of cell's content.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::layout::Constraint;
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::text::Line;
    /// use ratatui_logline_table::Table;
    ///
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let table = Table::new(&rows, widths, to_row).style(Style::new().red().italic());
    /// ```
    ///
    /// `Table` also implements the [`Styled`] trait, which means you can use style shorthands from
    /// the [`Stylize`] trait to set the style of the widget more concisely.
    ///
    /// ```rust
    /// use ratatui::layout::Constraint;
    /// use ratatui::style::Stylize;
    /// use ratatui::text::Line;
    /// use ratatui_logline_table::Table;
    ///
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let table = Table::new(&rows, widths, to_row).red().italic();
    /// ```
    ///
    /// [`Color`]: ratatui::style::Color
    /// [`Stylize`]: ratatui::style::Stylize
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Set the style of the selected row
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This style will be applied to the entire row, including the selection symbol if it is
    /// displayed, and will override any style set on the row or on the individual cells.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::{layout::Constraint, style::{Style, Stylize}, text::Line};
    /// # use ratatui_logline_table::Table;
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let table = Table::new(&rows, widths, to_row).row_highlight_style(Style::new().red().italic());
    /// ```
    /// [`Color`]: ratatui::style::Color
    pub fn row_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
        self.row_highlight_style = highlight_style.into();
        self
    }

    /// Set the symbol to be displayed in front of the selected row
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::{layout::Constraint, text::Line};
    /// # use ratatui_logline_table::Table;
    /// # let rows = [["Cell1", "Cell2"]];
    /// # let widths = [Constraint::Length(5), Constraint::Length(5)];
    /// # let to_row = |_index, line: &[&'static str; 2]| [Line::raw(line[0]), Line::raw(line[1])];
    /// let table = Table::new(&rows, widths, to_row).highlight_symbol(">>");
    /// ```
    pub fn highlight_symbol<T: Into<Text<'a>>>(mut self, highlight_symbol: T) -> Self {
        self.highlight_symbol = highlight_symbol.into();
        self
    }

    /// Set when to show the highlight spacing
    ///
    /// The highlight spacing is the spacing that is allocated for the selection symbol column (if
    /// enabled) and is used to shift the table when a row is selected. This method allows you to
    /// configure when this spacing is allocated.
    ///
    /// - [`HighlightSpacing::Always`] will always allocate the spacing, regardless of whether a row
    ///   is selected or not. This means that the table will never change size, regardless of if a
    ///   row is selected or not.
    /// - [`HighlightSpacing::WhenSelected`] will only allocate the spacing if a row is selected.
    ///   This means that the table will shift when a row is selected. This is the default setting
    ///   for backwards compatibility, but it is recommended to use `HighlightSpacing::Always` for a
    ///   better user experience.
    /// - [`HighlightSpacing::Never`] will never allocate the spacing, regardless of whether a row
    ///   is selected or not. This means that the highlight symbol will never be drawn.
    pub const fn highlight_spacing(mut self, value: HighlightSpacing) -> Self {
        self.highlight_spacing = value;
        self
    }

    /// Set how extra space is distributed amongst columns.
    ///
    /// This determines how the space is distributed when the constraints are satisfied. By default,
    /// the extra space is not distributed at all.  But this can be changed to distribute all extra
    /// space to the last column or to distribute it equally.
    pub const fn flex(mut self, flex: Flex) -> Self {
        self.flex = flex;
        self
    }
}

impl<const COLUMNS: usize, Logline> Widget for Table<'_, COLUMNS, Logline> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl<const COLUMNS: usize, Logline> Widget for &Table<'_, COLUMNS, Logline> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = State::new();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

impl<const COLUMNS: usize, Logline> StatefulWidget for Table<'_, COLUMNS, Logline> {
    type State = State;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl<const COLUMNS: usize, Logline> StatefulWidget for &Table<'_, COLUMNS, Logline> {
    type State = State;

    fn render(self, full_area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(full_area, self.style);
        if let Some(block) = &self.block {
            block.render(full_area, buf);
        }
        let table_area = self.block.inner_if_some(full_area);
        if table_area.is_empty() {
            state.last_row_area = Rect::ZERO;
            return;
        }

        state.last_biggest_index = self.lines.len().saturating_sub(1);

        if self.lines.is_empty() {
            state.selected = None;
        } else if state.selected.is_some_and(|row| row >= self.lines.len()) {
            state.selected = Some(state.last_biggest_index);
        }

        let selection_width = self.selection_width(state);
        let column_widths = self.get_column_widths(table_area.width, selection_width);
        let (header_area, rows_area, footer_area) = self.layout(table_area);

        state.last_row_area = rows_area;

        self.render_header(header_area, buf, &column_widths);

        self.render_rows(rows_area, buf, state, selection_width, &column_widths);

        self.render_footer(footer_area, buf, &column_widths);

        // Only render scrollbar when there is a border on the right
        if rows_area.right() < full_area.right() {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None);
            let overscroll_workaround = self.lines.len().saturating_sub(rows_area.height as usize);
            let mut scrollbar_state = ScrollbarState::new(overscroll_workaround)
                .position(state.offset)
                // Should be available_height but with the current overscroll workaround this looks nicer
                .viewport_content_length(rows_area.height as usize);
            let scrollbar_area = Rect {
                // Inner height to be exactly as the content
                y: rows_area.y,
                height: rows_area.height,
                // Outer width to stay on the right border
                x: full_area.x,
                width: full_area.width,
            };
            scrollbar.render(scrollbar_area, buf, &mut scrollbar_state);
        }
    }
}

// private methods for rendering
impl<const COLUMNS: usize, Logline> Table<'_, COLUMNS, Logline> {
    /// Splits the table area into a header, rows area and a footer
    fn layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let header_height = u16::from(self.header.is_some());
        let footer_height = u16::from(self.footer.is_some());
        Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .areas(area)
        .into()
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
        if let Some(ref header) = self.header {
            buf.set_style(area, self.header_style);
            for ((x, width), cell) in column_widths.iter().zip(header.iter()) {
                cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
            }
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
        if let Some(ref footer) = self.footer {
            buf.set_style(area, self.footer_style);
            for ((x, width), cell) in column_widths.iter().zip(footer.iter()) {
                cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
            }
        }
    }

    fn render_rows(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut State,
        selection_width: u16,
        columns_widths: &[(u16, u16)],
    ) {
        if self.lines.is_empty() {
            return;
        }

        // Scroll down offset as much as possible
        let offset_with_last_in_view = self.lines.len().saturating_sub(area.height as usize);
        let scroll_last_into_view = if let Some(selected) = state.selected {
            // Only scroll when the change will include both end and selection.
            // When the user manually scrolled away from the end keep the offset.
            selected >= offset_with_last_in_view
        } else {
            state.scroll_keeps_last_in_view || state.offset >= offset_with_last_in_view
        };
        if scroll_last_into_view {
            state.offset = offset_with_last_in_view;
            state.scroll_keeps_last_in_view = true;
        }

        let (start_index, end_index) = self.visible_rows(state, area);
        state.ensure_selected_in_view_on_next_render = false;
        state.offset = start_index;

        let mut selected_row_area = None;
        for (index, row) in self
            .lines
            .iter()
            .enumerate()
            .skip(start_index)
            .take(end_index - start_index)
        {
            let y = area.y + index.saturating_sub(start_index) as u16;
            let height = 1;
            let row_area = Rect { y, height, ..area };

            let is_selected = state.selected.is_some_and(|selected| selected == index);
            if selection_width > 0 && is_selected {
                let selection_area = Rect {
                    width: selection_width,
                    ..row_area
                };
                (&self.highlight_symbol).render(selection_area, buf);
            }
            let cells = (self.to_row)(index, row);
            for ((x, width), cell) in columns_widths.iter().copied().zip(cells.iter()) {
                cell.render(
                    Rect {
                        x: row_area.x + x,
                        width,
                        ..row_area
                    },
                    buf,
                );
            }
            if is_selected {
                selected_row_area = Some(row_area);
            }
        }

        if let Some(row_area) = selected_row_area {
            buf.set_style(row_area, self.row_highlight_style);
        }
    }

    /// Return the indexes of the visible rows.
    ///
    /// The algorithm works as follows:
    /// - start at the offset and calculate the height of the rows that can be displayed within the
    ///   area.
    /// - if the selected row is not visible, scroll the table to ensure it is visible.
    /// - if there is still space to fill then there's a partial row at the end which should be
    ///   included in the view.
    fn visible_rows(&self, state: &State, area: Rect) -> (usize, usize) {
        let mut start = state.offset.min(state.last_biggest_index);

        let ensure_index_in_view = state
            .selected
            .filter(|_| state.ensure_selected_in_view_on_next_render)
            .map(|selected| selected.min(state.last_biggest_index));

        if let Some(ensure_index_in_view) = ensure_index_in_view {
            start = start.min(ensure_index_in_view);
        }

        let mut height = area.height;
        let mut end = start
            .saturating_add(area.height as usize)
            .min(self.lines.len());

        if let Some(ensure_index_in_view) = ensure_index_in_view {
            // scroll down until the selected row is visible
            while ensure_index_in_view >= end {
                height = height.saturating_add(1);
                end += 1;
                while height > area.height {
                    height = height.saturating_sub(1);
                    start += 1;
                }
            }
        }

        // Include a partial row if there is space
        if height < area.height && end < self.lines.len() {
            end += 1;
        }

        (start, end)
    }

    /// Get all offsets and widths of all user specified columns.
    ///
    /// Returns (x, width). When self.widths is empty, it is assumed `.widths()` has not been called
    /// and a default of equal widths is returned.
    fn get_column_widths(&self, max_width: u16, selection_width: u16) -> Vec<(u16, u16)> {
        // this will always allocate a selection area
        let [_selection_area, columns_area] =
            Layout::horizontal([Constraint::Length(selection_width), Constraint::Fill(0)])
                .areas(Rect::new(0, 0, max_width, 1));
        let rects = Layout::horizontal(self.widths)
            .flex(self.flex)
            .spacing(self.column_spacing)
            .split(columns_area);
        rects.iter().map(|rect| (rect.x, rect.width)).collect()
    }

    /// Returns the width of the selection column if a row is selected, or the `highlight_spacing`
    /// is set to show the column always, otherwise 0.
    fn selection_width(&self, state: &State) -> u16 {
        let has_selection = state.selected.is_some();
        if self.highlight_spacing.should_add(has_selection) {
            self.highlight_symbol.width() as u16
        } else {
            0
        }
    }
}

fn ensure_percentages_less_than_100(widths: &[Constraint]) {
    for width in widths {
        if let Constraint::Percentage(percent) = width {
            assert!(
                *percent <= 100,
                "Percentages should be between 0 and 100 inclusively."
            );
        }
    }
}

impl<const COLUMNS: usize, Logline> Styled for Table<'_, COLUMNS, Logline> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}
