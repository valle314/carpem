use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use chrono::NaiveDateTime;
use ratatui::{
    prelude::*, widgets::{
        Block, Padding, Paragraph 
    }
};
use crate::{action::RoutedAction, config::Config};
use crate::{
    action::Action, components::Component, tui::Tui 
};
use super::ComponentName;

pub struct SelectedDate {
    selected_date: Option<NaiveDateTime>,
    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
}

impl SelectedDate {
    pub fn new() -> Self {
        Self { 
            selected_date: None,
            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
        }
    }

    pub fn update_date(&mut self, new_date: NaiveDateTime) {
        self.selected_date = Some(new_date);
    }
}

impl Component for SelectedDate {
    fn name() -> ComponentName {
        ComponentName::SelectedDate
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, _tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::CurrentDateUpdated(date) => self.update_date(*date),
            _ => {}
        }
        Ok(vec![])
    }

    fn register_config_handler(&mut self, config: Rc<RefCell<Config>>) -> Result<()> {
        let user_data = &config.try_borrow_mut()?.user_data;
        self.text_color = user_data.text;
        self.border_focused_color = user_data.border_focused;
        self.border_default_color = user_data.border_default;
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, is_focused: bool) {
        let style = match is_focused {
            true => Style::default().fg(self.border_focused_color),
            false => Style::default().fg(self.border_default_color)
        };

        let block = Block::bordered()
            .style(Style::new()).title("selected date").border_style(style);

        let date_format: &str = "%d-%m-%Y %H:%M:%S";
        let date_to_show = match self.selected_date {
            Some(current_date) => current_date.format(date_format).to_string(),
            None => "no date selected".to_string()
        };

        let date_len = date_to_show.len();
        let lines = vec![Line::from(date_to_show), Line::from("_".repeat(date_len + 2))];
        let lines_count = lines.len();
        let paragraph = Paragraph::new(lines).block(block
            .padding(Padding::new(
                    0, 
                    0,
                    ((area.height - 2) - lines_count as u16) / 2, // border takes up 2 
                    0,
            ))).alignment(Alignment::Center)
            .style(Style::default().fg(self.text_color));

        frame.render_widget(paragraph, area);
    }
}
