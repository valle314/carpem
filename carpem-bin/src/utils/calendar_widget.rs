use std::collections::HashMap;
use chrono::{Datelike, Days, NaiveDateTime};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use ratatui::widgets::block::{Block, BlockExt};
use ratatui::widgets::WidgetRef;

/// Display a month calendar for the month containing `display_date`
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
// pub struct Monthly<'a, DS: DateStyler<Tz>> {
pub struct Monthly<'a, DS> 
where
    DS: DateStyler
{
    pub display_date: NaiveDateTime,
    pub events: DS,
    show_surrounding: Option<Style>,
    pub show_weekday: Option<Style>,
    pub show_month: Option<Style>,
    pub iso_week_style: Style,
    pub default_style: Style,
    pub block: Option<Block<'a>>,
}

// impl<'a, Tz: TimeZone, DS: DateStyler<Tz>> Monthly<'a, Tz, DS> 
impl<'a, DS> Monthly<'a, DS>
where
    DS: DateStyler,
{
    /// Construct a calendar for the `display_date` and highlight the `events`
    pub const fn new(display_date: NaiveDateTime, events: DS) -> Self {
        Self {
            display_date,
            events,
            show_surrounding: None,
            show_weekday: None,
            iso_week_style: Style::new(),
            show_month: None,
            default_style: Style::new(),
            block: None,
        }
    }

    /// Fill the calendar slots for days not in the current month also, this causes each line to be
    /// completely filled. If there is an event style for a date, this style will be patched with
    /// the event's style
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_surrounding<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_surrounding = Some(style.into());
        self
    }

    /// Display a header containing weekday abbreviations
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_weekdays_header<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_weekday = Some(style.into());
        self
    }

    /// Display a header containing the month and year
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_month_header<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_month = Some(style.into());
        self
    }

    /// How to render otherwise unstyled dates
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn default_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.default_style = style.into();
        self
    }

    /// Render the calendar within a [Block]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Return a style with only the background from the default style
    const fn default_bg(&self) -> Style {
        match self.default_style.bg {
            None => Style::new(),
            Some(c) => Style::new().bg(c),
        }
    }

    /// All logic to style a date goes here.
    fn format_date(&self, date: NaiveDateTime) -> Span {
        if date.month() == self.display_date.month() {
            Span::styled(
                format!("{:2?}", date.day()),
                self.default_style.patch(self.events.get_style(&date, &self.display_date)),
            )
        } else {
            match self.show_surrounding {
                None => Span::styled("  ", self.default_bg()),
                Some(s) => {
                    let style = self
                        .default_style
                        .patch(s)
                        .patch(self.events.get_style(&date, &self.display_date));
                    Span::styled(format!("{:2?}", date.day()), style)
                }
            }
        }
    }
}

impl<DS> WidgetRef for Monthly<'_, DS>
where
    DS: DateStyler
{
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if let Some(block) = self.block.as_ref() {
            block.render(area, buf);
        };
        let inner = self.block.inner_if_some(area);
        self.render_monthly(inner, buf);
    }
}

impl<DS> WidgetRef for &Monthly<'_, DS>
where
    DS: DateStyler
{
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        WidgetRef::render_ref(*self, area, buf);
    }
}

// impl<DS> Widget for Monthly<'_, DS>
// where
//     DS: DateStyler
// {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         Widget::render(&self, area, buf);
//     }
// }

// impl<DS> Widget for &Monthly<'_, DS>
// where
//     DS: DateStyler
// {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         if let Some(block) = self.block.as_ref() {
//             block.render(area, buf);
//         };
//         let inner = self.block.inner_if_some(area);
//         self.render_monthly(inner, buf);
//     }
// }

impl<DS> Monthly<'_, DS> 
where
    DS: DateStyler
{
    fn render_monthly(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(self.show_month.is_some().into()),
            Constraint::Length(self.show_weekday.is_some().into()),
            Constraint::Fill(1),
        ]);
        let [month_header, mut days_header, mut days_area] = layout.areas(area);

        // draw the month name and year
        let month_name = self.display_date.format("%B").to_string();
        if let Some(style) = self.show_month {
            Line::styled(
                format!("{}({}) {}", month_name, self.display_date.month(), self.display_date.year()),
                style.bold(),
            )
            .alignment(Alignment::Center)
            .render(month_header, buf);
        }

        let mut days_aligned = false;

        // draw days of week
        if let Some(style) = self.show_weekday {
            let span = Span::styled("   Mo Tu We Th Fr Sa Su", style).bold(); // align after "ww "
            [days_header] = Layout::horizontal([Constraint::Length(span.content.len() as u16)])
                .flex(ratatui::layout::Flex::Center).areas(days_header);
            span.render(days_header, buf);
        }

        let first_of_month = self.display_date.with_day(1).unwrap_or_else(|| self.display_date);
        let offset = first_of_month.weekday().number_from_monday() - 1;
        let mut curr_day = match first_of_month.clone().checked_sub_days(Days::new(offset as u64)) {
            Some(d) => d,
            None => return
        };

        let mut y = days_area.y;
        let mut iso_week = first_of_month.iso_week().week();

        let mut break_outer = false;
        while curr_day.month0() != (self.display_date.month0() + 1) % 12 {
            let mut spans = Vec::with_capacity(14);
            for i in 0..7 {
                if i == 0 {
                    spans.push(Span::styled(format!("{:2?}", iso_week), self.iso_week_style)); 
                    spans.push(Span::styled(" ", Style::default())); 
                } else {
                    spans.push(Span::styled(" ", self.default_bg()));
                }
                spans.push(self.format_date(curr_day.clone()));
                curr_day = match curr_day.checked_add_days(Days::new(1)) {
                    Some(d) => d,
                    None => {
                        break_outer = true;
                        break;
                    }
                };
            }
            if !days_aligned {
                let mut total_span_width = 0;
                for s in spans.iter() {
                    total_span_width += s.content.len();
                }
                [days_area] = Layout::horizontal([Constraint::Length(total_span_width as u16)]) 
                    .flex(ratatui::layout::Flex::Center)
                        .areas(days_area);
                days_aligned = true;
            }

            if iso_week >= 52 {
                iso_week = curr_day.iso_week().week();
            } else { iso_week += 1; }
            if buf.area.height > y {
                buf.set_line(days_area.x, y, &spans.into(), area.width);
            }
            y += 1;
            if break_outer { break; }
        }
    }
}

/// Provides a method for styling a given date. [Monthly] is generic on this trait, so any type
/// that implements this trait can be used.

// pub trait DateStyler {
//     /// Given a date, return a style for that date
//     fn get_style(&self, date: DateTime<Utc>) -> Style;
// }

pub trait DateStyler {
    /// Given a date, return a style for that date
    fn get_style(&self, date: &NaiveDateTime, selected_date: &NaiveDateTime) -> Style;
}

#[derive(Debug, Clone, Eq, PartialEq)]
// pub struct CalendarEventStore(pub HashMap<NaiveDateTime, Style>);
pub struct CalendarEventStore {
    pub store: HashMap<NaiveDateTime, Style>,
    pub display_date_style: Style,
}

#[allow(unused)]
impl CalendarEventStore {
    pub fn add<S: Into<Style>>(&mut self, date: NaiveDateTime, style: S) {
        let _ = self.store.insert(date, style.into());
    }

    pub fn remove(&mut self, date: &NaiveDateTime) {
        let _ = self.store.remove(date);
    }

    fn lookup_style(&self, date: &NaiveDateTime) -> Style {
        self.store.get(&date).copied().unwrap_or_default()
    }
}

impl DateStyler for CalendarEventStore {
    fn get_style(&self, date: &NaiveDateTime, selected_date: &NaiveDateTime) -> Style {
        if date == selected_date {
            self.display_date_style
        } else {
            self.lookup_style(date)
        }
    }
}

impl Default for CalendarEventStore {
    fn default() -> Self {
        Self { 
            store: HashMap::with_capacity(4), 
            display_date_style: Style::default()
                .bg(ratatui::style::Color::Yellow)
                .fg(ratatui::style::Color::Black)
        }
    }
}
