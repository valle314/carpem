//! The [`Table`] widget is used to display multiple rows and columns in a grid and allows selecting
//! one or multiple cells.

use itertools::Itertools;
use std::vec;
use std::vec::Vec;

use ratatui::prelude::buffer::Buffer;
use ratatui::prelude::layout::{Constraint, Flex, Layout, Rect};
use ratatui::prelude::style::{Style, Styled};
use ratatui::prelude::text::Text;
use ratatui::widgets::{StatefulWidget, Widget, WidgetRef, StatefulWidgetRef};
use ratatui::widgets::block::{Block, BlockExt};



#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Cell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> Cell<'a> {
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }

    pub fn content<T>(mut self, content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        self.content = content.into();
        self
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl Cell<'_> {
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        Widget::render(&self.content, area, buf);
    }
}

impl<'a, T> From<T> for Cell<'a>
where
    T: Into<Text<'a>>,
{
    fn from(content: T) -> Self {
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }
}

impl Styled for Cell<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}


#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct TableState {
    pub offset: usize,
    pub selected: Option<usize>,
    pub selected_column: Option<usize>,
}

impl TableState {
    pub const fn new() -> Self {
        Self {
            offset: 0,
            selected: None,
            selected_column: None,
        }
    }

    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_selected<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<usize>>,
    {
        self.selected = selected.into();
        self
    }

    pub fn with_selected_column<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<usize>>,
    {
        self.selected_column = selected.into();
        self
    }

    pub fn with_selected_cell<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<(usize, usize)>>,
    {
        if let Some((r, c)) = selected.into() {
            self.selected = Some(r);
            self.selected_column = Some(c);
        } else {
            self.selected = None;
            self.selected_column = None;
        }

        self
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub const fn selected_column(&self) -> Option<usize> {
        self.selected_column
    }

    pub const fn selected_cell(&self) -> Option<(usize, usize)> {
        if let (Some(r), Some(c)) = (self.selected, self.selected_column) {
            return Some((r, c));
        }
        None
    }

    pub fn selected_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected
    }

   /// ```
    pub fn selected_column_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected_column
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub fn select_column(&mut self, index: Option<usize>) {
        self.selected_column = index;
    }

    pub fn select_cell(&mut self, indexes: Option<(usize, usize)>) {
        if let Some((r, c)) = indexes {
            self.selected = Some(r);
            self.selected_column = Some(c);
        } else {
            self.offset = 0;
            self.selected = None;
            self.selected_column = None;
        }
    }

    pub fn select_next(&mut self) {
        let next = self.selected.map_or(0, |i| i.saturating_add(1));
        self.select(Some(next));
    }

    pub fn select_next_column(&mut self) {
        let next = self.selected_column.map_or(0, |i| i.saturating_add(1));
        self.select_column(Some(next));
    }

    pub fn select_previous(&mut self) {
        let previous = self.selected.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select(Some(previous));
    }

    pub fn select_previous_column(&mut self) {
        let previous = self
            .selected_column
            .map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select_column(Some(previous));
    }

    pub fn select_first(&mut self) {
        self.select(Some(0));
    }

    pub fn select_first_column(&mut self) {
        self.select_column(Some(0));
    }

    pub fn select_last(&mut self) {
        self.select(Some(usize::MAX));
    }

    pub fn select_last_column(&mut self) {
        self.select_column(Some(usize::MAX));
    }

    pub fn scroll_down_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_add(amount as usize)));
    }

    pub fn scroll_up_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_sub(amount as usize)));
    }

    pub fn scroll_right_by(&mut self, amount: u16) {
        let selected = self.selected_column.unwrap_or_default();
        self.select_column(Some(selected.saturating_add(amount as usize)));
    }

    pub fn scroll_left_by(&mut self, amount: u16) {
        let selected = self.selected_column.unwrap_or_default();
        self.select_column(Some(selected.saturating_sub(amount as usize)));
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Row<'a> {
    pub cells: Vec<Cell<'a>>,
    pub height: u16,
    pub top_margin: u16,
    pub bottom_margin: u16,
    pub style: Style,
}

impl<'a> Row<'a> {
    pub fn new<T>(cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<Cell<'a>>,
    {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
            height: 1,
            ..Default::default()
        }
    }

    pub fn cells<T>(mut self, cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<Cell<'a>>,
    {
        self.cells = cells.into_iter().map(Into::into).collect();
        self
    }

    pub const fn height(mut self, height: u16) -> Self {
        self.height = height;
        self
    }

    pub const fn top_margin(mut self, margin: u16) -> Self {
        self.top_margin = margin;
        self
    }

    pub const fn bottom_margin(mut self, margin: u16) -> Self {
        self.bottom_margin = margin;
        self
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

// private methods for rendering
impl Row<'_> {
    /// Returns the total height of the row.
    pub(crate) const fn height_with_margin(&self) -> u16 {
        self.height
            .saturating_add(self.top_margin)
            .saturating_add(self.bottom_margin)
    }
}

impl Styled for Row<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl<'a, Item> FromIterator<Item> for Row<'a>
where
    Item: Into<Cell<'a>>,
{
    fn from_iter<IterCells: IntoIterator<Item = Item>>(cells: IterCells) -> Self {
        Self::new(cells)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Table<'a> {
    /// Data to display in each row
    pub rows: Vec<Row<'a>>,

    /// Optional header
    header: Option<Row<'a>>,

    /// Optional footer
    footer: Option<Row<'a>>,

    /// Display relative line number
    pub show_relative_line_number: bool,

    /// Width constraints for each column
    widths: Vec<Constraint>,

    /// Space between each column
    column_spacing: u16,

    /// A block to wrap the widget in
    pub block: Option<Block<'a>>,

    /// Base style for the widget
    style: Style,

    /// Style used to render the selected row
    row_highlight_style: Style,

    /// Style used to render the selected column
    column_highlight_style: Style,

    /// Style used to render the selected cell
    cell_highlight_style: Style,

    /// Symbol in front of the selected row
    highlight_symbol: Text<'a>,

    /// Controls how to distribute extra space among the columns
    flex: Flex,
}

impl Default for Table<'_> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            header: None,
            footer: None,
            show_relative_line_number: false,
            widths: Vec::new(),
            column_spacing: 1,
            block: None,
            style: Style::new(),
            row_highlight_style: Style::new(),
            column_highlight_style: Style::new(),
            cell_highlight_style: Style::new(),
            highlight_symbol: Text::default(),
            flex: Flex::Start,
        }
    }
}

impl<'a> Table<'a> {
    pub fn new<R, C>(rows: R, widths: C) -> Self
    where
        R: IntoIterator,
        R::Item: Into<Row<'a>>,
        C: IntoIterator,
        C::Item: Into<Constraint>,
    {
        let widths = widths.into_iter().map(Into::into).collect_vec();
        ensure_percentages_less_than_100(&widths);

        let rows = rows.into_iter().map(Into::into).collect();
        Self {
            rows,
            widths,
            ..Default::default()
        }
    }

    pub fn rows<T>(mut self, rows: T) -> Self
    where
        T: IntoIterator<Item = Row<'a>>,
    {
        self.rows = rows.into_iter().collect();
        self
    }

    pub fn header(mut self, header: Row<'a>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn show_relative_line_number(mut self, show_relative_line_number: bool) -> Self {
        self.show_relative_line_number = show_relative_line_number;
        self
    }

    pub fn footer(mut self, footer: Row<'a>) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn widths<I>(mut self, widths: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        let widths = widths.into_iter().map(Into::into).collect_vec();
        ensure_percentages_less_than_100(&widths);
        self.widths = widths;
        self
    }

    pub const fn column_spacing(mut self, spacing: u16) -> Self {
        self.column_spacing = spacing;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    pub fn row_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
        self.row_highlight_style = highlight_style.into();
        self
    }

    pub fn column_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
        self.column_highlight_style = highlight_style.into();
        self
    }

    pub fn cell_highlight_style<S: Into<Style>>(mut self, highlight_style: S) -> Self {
        self.cell_highlight_style = highlight_style.into();
        self
    }

    pub fn highlight_symbol<T: Into<Text<'a>>>(mut self, highlight_symbol: T) -> Self {
        self.highlight_symbol = highlight_symbol.into();
        self
    }

    pub const fn flex(mut self, flex: Flex) -> Self {
        self.flex = flex;
        self
    }
}

impl Widget for Table<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Table<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = TableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

impl StatefulWidget for Table<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl StatefulWidget for &Table<'_> {
    type State = TableState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidgetRef::render_ref(self, area, buf, state);
    }
}

impl StatefulWidgetRef for Table<'_> {
    type State = TableState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        self.block.render_ref(area, buf);
        let table_area = self.block.inner_if_some(area);
        if table_area.is_empty() {
            return;
        }

        if state.selected.is_some_and(|s| s >= self.rows.len()) {
            state.select(Some(self.rows.len().saturating_sub(1)));
        }

        if self.rows.is_empty() {
            state.select(None);
        }

        let column_count = self.column_count();
        if state.selected_column.is_some_and(|s| s >= column_count) {
            state.select_column(Some(column_count.saturating_sub(1)));
        }
        if column_count == 0 {
            state.select_column(None);
        }

        let selection_width = self.selection_width(state);
        let columns_widths =
            self.get_column_widths(table_area.width, selection_width, column_count);
        let (header_area, rows_area, footer_area) = self.layout(table_area);

        self.render_header(header_area, buf, &columns_widths);

        self.render_rows(
            rows_area,
            buf,
            state,
            selection_width,
            &columns_widths,
        );

        self.render_footer(footer_area, buf, &columns_widths);
    }
}


// private methods for rendering
impl Table<'_> {
    /// Splits the table area into a header, rows area and a footer
    fn layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let header_top_margin = self.header.as_ref().map_or(0, |h| h.top_margin);
        let header_height = self.header.as_ref().map_or(0, |h| h.height);
        let header_bottom_margin = self.header.as_ref().map_or(0, |h| h.bottom_margin);
        let footer_top_margin = self.footer.as_ref().map_or(0, |h| h.top_margin);
        let footer_height = self.footer.as_ref().map_or(0, |f| f.height);
        let footer_bottom_margin = self.footer.as_ref().map_or(0, |h| h.bottom_margin);
        let layout = Layout::vertical([
            Constraint::Length(header_top_margin),
            Constraint::Length(header_height),
            Constraint::Length(header_bottom_margin),
            Constraint::Min(0),
            Constraint::Length(footer_top_margin),
            Constraint::Length(footer_height),
            Constraint::Length(footer_bottom_margin),
        ])
        .split(area);
        let (header_area, rows_area, footer_area) = (layout[1], layout[3], layout[5]);
        (header_area, rows_area, footer_area)
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
        if let Some(ref header) = self.header {
            buf.set_style(area, header.style);

            let mut columns_widths_iterator = column_widths.iter();
            if self.show_relative_line_number {
                let _ = columns_widths_iterator.next(); // ignore line number header title
            }

            for (_header_index, ((x, width), cell)) in columns_widths_iterator.zip(header.cells.iter()).enumerate() {
                cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
            }
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer, column_widths: &[(u16, u16)]) {
        if let Some(ref footer) = self.footer {
            buf.set_style(area, footer.style);
            for ((x, width), cell) in column_widths.iter().zip(footer.cells.iter()) {
                cell.render(Rect::new(area.x + x, area.y, *width, area.height), buf);
            }
        }
    }

    fn render_rows(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut TableState,
        selection_width: u16,
        columns_widths: &[(u16, u16)],
    ) {
        if self.rows.is_empty() {
            return;
        }

        let (start_index, end_index) = self.visible_rows(state, area);
        state.offset = start_index;

        let mut y_offset = 0;
        let mut selected_row_area = None;

        for (i, row) in self
            .rows
            .iter()
            .enumerate()
            .skip(start_index)
            .take(end_index - start_index)
        {
            let y = area.y + y_offset + row.top_margin;
            let height = (y + row.height).min(area.bottom()).saturating_sub(y);
            let row_area = Rect { y, height, ..area };
            buf.set_style(row_area, row.style);

            let is_selected = state.selected.is_some_and(|index| index == i);
            if selection_width > 0 && is_selected {
                let selection_area = Rect {
                    width: selection_width,
                    ..row_area
                };
                buf.set_style(selection_area, row.style);
                (&self.highlight_symbol).render(selection_area, buf);
            }

            let mut columns_widths_iterator = columns_widths.iter();
            if self.show_relative_line_number {
                if let Some((_x, width)) = columns_widths_iterator.next() {
                    let current_selection_index = state.selected.unwrap_or_else(|| 0); // for relative index
                    let relative_line_number = match current_selection_index.abs_diff(i) {
                        0 => current_selection_index,
                        n => n
                    };

                    Cell::new(relative_line_number.to_string()).render(
                        Rect::new(row_area.x, row_area.y, *width, row_area.height),
                        buf,
                    );
                }
            }

            for (_col_index, ((x, width), cell)) in columns_widths_iterator.zip(row.cells.iter()).enumerate() {
                cell.render(
                    Rect::new(row_area.x + x, row_area.y, *width, row_area.height),
                    buf,
                );
            }

            if is_selected {
                selected_row_area = Some(row_area);
            }
            y_offset += row.height_with_margin();
        }

        let selected_column_area = state.selected_column.and_then(|s| {
            // The selection is clamped by the column count. Since a user can manually specify an
            // incorrect number of widths, we should use panic free methods.
            columns_widths.get(s).map(|(x, width)| Rect {
                x: x + area.x,
                width: *width,
                ..area
            })
        });

        match (selected_row_area, selected_column_area) {
            (Some(row_area), Some(col_area)) => {
                buf.set_style(row_area, self.row_highlight_style);
                buf.set_style(col_area, self.column_highlight_style);
                let cell_area = row_area.intersection(col_area);
                buf.set_style(cell_area, self.cell_highlight_style);
            }
            (Some(row_area), None) => {
                buf.set_style(row_area, self.row_highlight_style);
            }
            (None, Some(col_area)) => {
                buf.set_style(col_area, self.column_highlight_style);
            }
            (None, None) => (),
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
    fn visible_rows(&self, state: &TableState, area: Rect) -> (usize, usize) {
        let last_row = self.rows.len().saturating_sub(1);
        let mut start = state.offset.min(last_row);

        if let Some(selected) = state.selected {
            start = start.min(selected);
        }

        let mut end = start;
        let mut height = 0;

        for item in self.rows.iter().skip(start) {
            if height + item.height > area.height {
                break;
            }
            height += item.height_with_margin();
            end += 1;
        }

        if let Some(selected) = state.selected {
            let selected = selected.min(last_row);

            // scroll down until the selected row is visible
            while selected >= end {
                height = height.saturating_add(self.rows[end].height_with_margin());
                end += 1;
                while height > area.height {
                    height = height.saturating_sub(self.rows[start].height_with_margin());
                    start += 1;
                }
            }
        }

        // Include a partial row if there is space
        if height < area.height && end < self.rows.len() {
            end += 1;
        }

        (start, end)
    }

    /// Get all offsets and widths of all user specified columns.
    ///
    /// Returns (x, width). When self.widths is empty, it is assumed `.widths()` has not been called
    /// and a default of equal widths is returned.
    fn get_column_widths(
        &self,
        max_width: u16,
        selection_width: u16,
        col_count: usize,
    ) -> Vec<(u16, u16)> {
        let mut widths = if self.widths.is_empty() {
            // Divide the space between each column equally
            vec![Constraint::Length(max_width / col_count.max(1) as u16); col_count]
        } else {
            self.widths.clone()
        };

        if self.show_relative_line_number {
            let line_number_width = self.rows.len().to_string().len() as u16;
            widths.insert(0, Constraint::Length(line_number_width));
        }

        // this will always allocate a selection a
        let [_selection_area, columns_area] =
            Layout::horizontal([Constraint::Length(selection_width), Constraint::Fill(0)])
                .areas(Rect::new(0, 0, max_width, 1));
        let rects = Layout::horizontal(widths)
            .flex(self.flex)
            .spacing(self.column_spacing)
            .split(columns_area);
        rects.iter().map(|c| (c.x, c.width)).collect()
    }

    fn column_count(&self) -> usize {
        self.rows
            .iter()
            .chain(self.footer.iter())
            .chain(self.header.iter())
            .map(|r| r.cells.len())
            .max()
            .unwrap_or_default()
    }

    /// Returns the width of the selection column if a row is selected, or the `highlight_spacing`
    /// is set to show the column always, otherwise 0.
    fn selection_width(&self, _state: &TableState) -> u16 {
        // let has_selection = state.selected.is_some();
        self.highlight_symbol.width() as u16
    }
}

fn ensure_percentages_less_than_100(widths: &[Constraint]) {
    for w in widths {
        if let Constraint::Percentage(p) = w {
            assert!(
                *p <= 100,
                "Percentages should be between 0 and 100 inclusively."
            );
        }
    }
}

impl Styled for Table<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl<'a, Item> FromIterator<Item> for Table<'a>
where
    Item: Into<Row<'a>>,
{
    /// Collects an iterator of rows into a table.
    ///
    /// When collecting from an iterator into a table, the user must provide the widths using
    /// `Table::widths` after construction.
    fn from_iter<Iter: IntoIterator<Item = Item>>(rows: Iter) -> Self {
        let widths: [Constraint; 0] = [];
        Self::new(rows, widths)
    }
}
