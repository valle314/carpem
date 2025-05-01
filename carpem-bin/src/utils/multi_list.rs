use ratatui::{
    buffer::Buffer, layout::Rect, prelude::BlockExt, style::{Style, Styled}, text::{Line, Text, ToLine}, widgets::{
        Block,
        StatefulWidget,
        Widget,
    }
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ListItem<'a> {
    pub(crate) content: Text<'a>,
    pub(crate) style: Style,
}

impl<'a> ListItem<'a> {
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    pub fn height(&self) -> usize {
        self.content.height()
    }

    pub fn width(&self) -> usize {
        self.content.width()
    }
}

impl<'a, T> From<T> for ListItem<'a>
where
    T: Into<Text<'a>>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct List<'a> {
    /// An optional block to wrap the widget in
    pub(crate) block: Option<Block<'a>>,
    /// The items in the list
    pub(crate) items: Vec<ListItem<'a>>,
    /// Style used as a base style for the widget
    pub(crate) style: Style,
    /// List display direction
    pub(crate) direction: ListDirection,
    /// Style used to render selected item
    pub(crate) highlight_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    pub(crate) highlight_symbol: Option<Line<'a>>,
    /// Whether to repeat the highlight symbol for each line of the selected item
    pub(crate) repeat_highlight_symbol: bool,
    /// How many items to try to keep visible before and after the selected item
    pub(crate) scroll_padding: usize,

    pub(crate) select_style: Style
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ListDirection {
    /// The first value is on the top, going to the bottom
    #[default]
    TopToBottom,
    /// The first value is on the bottom, going to the top.
    BottomToTop,
}

impl<'a> List<'a> {
    pub fn new<T>(items: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<ListItem<'a>>,
    {
        Self {
            block: None,
            style: Style::default(),
            items: items.into_iter().map(Into::into).collect(),
            direction: ListDirection::default(),
            ..Self::default()
        }
    }

    pub fn items<T>(mut self, items: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<ListItem<'a>>,
    {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    pub fn highlight_symbol<L: Into<Line<'a>>>(mut self, highlight_symbol: L) -> Self {
        self.highlight_symbol = Some(highlight_symbol.into());
        self
    }

    pub fn highlight_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.highlight_style = style.into();
        self
    }

    pub fn select_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.select_style = style.into();
        self
    }


    pub const fn repeat_highlight_symbol(mut self, repeat: bool) -> Self {
        self.repeat_highlight_symbol = repeat;
        self
    }

    pub const fn direction(mut self, direction: ListDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn scroll_padding(mut self, padding: usize) -> Self {
        self.scroll_padding = padding;
        self
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Styled for List<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Styled for ListItem<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl<'a, Item> FromIterator<Item> for List<'a>
where
    Item: Into<ListItem<'a>>,
{
    fn from_iter<Iter: IntoIterator<Item = Item>>(iter: Iter) -> Self {
        Self::new(iter)
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ListState {
    pub offset: usize,
    pub current_index: Option<usize>,
    pub selected_indices: Vec<usize>
}

impl ListState {
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub const fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.current_index = selected;
        self
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    pub const fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn current_index_mut(&mut self) -> &mut Option<usize> {
        &mut self.current_index
    }

    pub fn selected_indices(&mut self) -> &mut Vec<usize> {
        &mut self.selected_indices
    }

    pub fn set_current_index(&mut self, index: Option<usize>) {
        self.current_index = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub fn next(&mut self) {
        let next = self.current_index.map_or(0, |i| i.saturating_add(1));
        self.set_current_index(Some(next));
    }

    pub fn previous(&mut self) {
        let previous = self.current_index.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.set_current_index(Some(previous));
    }

    pub fn highlight_first(&mut self) {
        self.set_current_index(Some(0));
    }

    pub fn highlight_last(&mut self) {
        self.set_current_index(Some(usize::MAX));
    }

    pub fn scroll_down_by(&mut self, amount: u16) {
        let selected = self.current_index.unwrap_or_default();
        self.set_current_index(Some(selected.saturating_add(amount as usize)));
    }

    pub fn scroll_up_by(&mut self, amount: u16) {
        let selected = self.current_index.unwrap_or_default();
        self.set_current_index(Some(selected.saturating_sub(amount as usize)));
    }

    pub fn select_current(&mut self) {
        if let Some(current_index) = self.current_index {
            if !self.selected_indices.contains(&current_index) {
                self.selected_indices.push(current_index);
            }
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if !self.selected_indices.contains(&index) {
            self.selected_indices.push(index);
        }
    }


    pub fn toggle(&mut self) -> bool {
        if let Some(current_index) = self.current_index {
            if let Some(index) = self.selected_indices.iter().position(|&i| i == current_index) {
                self.selected_indices.remove(index);
                return false;
            } else {
                self.selected_indices.push(current_index);
                return true;
            }
        }
        return false;
    }
}  



















































impl Widget for List<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &List<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

impl StatefulWidget for List<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl StatefulWidget for &List<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        self.block.render(area, buf);
        let list_area = self.block.inner_if_some(area);

        if list_area.is_empty() {
            return;
        }

        if self.items.is_empty() {
            state.set_current_index(None);
            return;
        }

        // If the selected index is out of bounds, set it to the last item
        if state.current_index.is_some_and(|s| s >= self.items.len()) {
            state.set_current_index(Some(self.items.len().saturating_sub(1)));
        }

        let list_height = list_area.height as usize;

        let (first_visible_index, last_visible_index) =
            self.get_items_bounds(state.current_index, state.offset, list_height);

        // Important: this changes the state's offset to be the beginning of the now viewable items
        state.offset = first_visible_index;

        // Get our set highlighted symbol (if one was set)
        let default_highlight_symbol = Line::default();
        let highlight_symbol = self
            .highlight_symbol
            .as_ref()
            .unwrap_or_else(|| &default_highlight_symbol);
        let highlight_symbol_width = highlight_symbol.width() as u16;
        let empty_symbol = " ".repeat(highlight_symbol_width as usize);
        let empty_symbol = empty_symbol.to_line();

        let mut current_height = 0;
        let selection_spacing = state.current_index.is_some();
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(last_visible_index - first_visible_index)
        {
            let (x, y) = if self.direction == ListDirection::BottomToTop {
                current_height += item.height() as u16;
                (list_area.left(), list_area.bottom() - current_height)
            } else {
                let pos = (list_area.left(), list_area.top() + current_height);
                current_height += item.height() as u16;
                pos
            };

            let row_area = Rect::new(x, y, list_area.width, item.height() as u16);

            let item_style = self.style.patch(item.style);
            buf.set_style(row_area, item_style);


            let item_area = if selection_spacing {
                Rect {
                    x: row_area.x + highlight_symbol_width,
                    width: row_area.width.saturating_sub(highlight_symbol_width),
                    ..row_area
                }
            } else {
                row_area
            };
            Widget::render(&item.content, item_area, buf);

            let is_highlighted = state.current_index == Some(i);
            if is_highlighted {
                buf.set_style(row_area, self.highlight_style);
            }

            let select_style = self.select_style;
            let is_selected = state.selected_indices.contains(&i);
            if is_selected {
                buf.set_style(row_area, select_style);
            }



            if selection_spacing {
                for j in 0..item.content.height() {
                    // if the item is selected, we need to display the highlight symbol:
                    // - either for the first line of the item only,
                    // - or for each line of the item if the appropriate option is set
                    let line = if is_highlighted && (j == 0 || self.repeat_highlight_symbol) {
                        highlight_symbol
                    } else {
                        &empty_symbol
                    };
                    let highlight_area = Rect::new(x, y + j as u16, highlight_symbol_width, 1);
                    line.render(highlight_area, buf);
                }
            }
        }
    }
}

impl List<'_> {
    /// Given an offset, calculate which items can fit in a given area
    fn get_items_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: usize,
    ) -> (usize, usize) {
        let offset = offset.min(self.items.len().saturating_sub(1));

        // Note: visible here implies visible in the given area
        let mut first_visible_index = offset;
        let mut last_visible_index = offset;

        // Current height of all items in the list to render, beginning at the offset
        let mut height_from_offset = 0;

        // Calculate the last visible index and total height of the items
        // that will fit in the available space
        for item in self.items.iter().skip(offset) {
            if height_from_offset + item.height() > max_height {
                break;
            }

            height_from_offset += item.height();

            last_visible_index += 1;
        }

        // Get the selected index and apply scroll_padding to it, but still honor the offset if
        // nothing is selected. This allows for the list to stay at a position after select()ing
        // None.
        let index_to_display = self
            .apply_scroll_padding_to_selected_index(
                selected,
                max_height,
                first_visible_index,
                last_visible_index,
            )
            .unwrap_or_else(|| offset);

        // Recall that last_visible_index is the index of what we
        // can render up to in the given space after the offset
        // If we have an item selected that is out of the viewable area (or
        // the offset is still set), we still need to show this item
        while index_to_display >= last_visible_index {
            height_from_offset =
                height_from_offset.saturating_add(self.items[last_visible_index].height());

            last_visible_index += 1;

            // Now we need to hide previous items since we didn't have space
            // for the selected/offset item
            while height_from_offset > max_height {
                height_from_offset =
                    height_from_offset.saturating_sub(self.items[first_visible_index].height());

                // Remove this item to view by starting at the next item index
                first_visible_index += 1;
            }
        }

        // Here we're doing something similar to what we just did above
        // If the selected item index is not in the viewable area, let's try to show the item
        while index_to_display < first_visible_index {
            first_visible_index -= 1;

            height_from_offset =
                height_from_offset.saturating_add(self.items[first_visible_index].height());

            // Don't show an item if it is beyond our viewable height
            while height_from_offset > max_height {
                last_visible_index -= 1;

                height_from_offset =
                    height_from_offset.saturating_sub(self.items[last_visible_index].height());
            }
        }

        (first_visible_index, last_visible_index)
    }

    /// Applies scroll padding to the selected index, reducing the padding value to keep the
    /// selected item on screen even with items of inconsistent sizes
    ///
    /// This function is sensitive to how the bounds checking function handles item height
    fn apply_scroll_padding_to_selected_index(
        &self,
        selected: Option<usize>,
        max_height: usize,
        first_visible_index: usize,
        last_visible_index: usize,
    ) -> Option<usize> {
        let last_valid_index = self.items.len().saturating_sub(1);
        let selected = selected?.min(last_valid_index);

        // The bellow loop handles situations where the list item sizes may not be consistent,
        // where the offset would have excluded some items that we want to include, or could
        // cause the offset value to be set to an inconsistent value each time we render.
        // The padding value will be reduced in case any of these issues would occur
        let mut scroll_padding = self.scroll_padding;
        while scroll_padding > 0 {
            let mut height_around_selected = 0;
            for index in selected.saturating_sub(scroll_padding)
                ..=selected
                    .saturating_add(scroll_padding)
                    .min(last_valid_index)
            {
                height_around_selected += self.items[index].height();
            }
            if height_around_selected <= max_height {
                break;
            }
            scroll_padding -= 1;
        }

        Some(
            if (selected + scroll_padding).min(last_valid_index) >= last_visible_index {
                selected + scroll_padding
            } else if selected.saturating_sub(scroll_padding) < first_visible_index {
                selected.saturating_sub(scroll_padding)
            } else {
                selected
            }
            .min(last_valid_index),
        )
    }
}
