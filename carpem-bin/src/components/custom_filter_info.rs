use std::{cell::RefCell, rc::Rc};
use anyhow::{Context, Result};
use ratatui::{
    prelude::*, widgets::{
        Block, Paragraph
    }
};
use crate::{
    action::{Action, RoutedAction}, app::Store, components::Component, config::Config, tui::Tui
};

use super::ComponentName;

pub struct CustomFilterInfo {
    store: Option<Rc<RefCell<Store>>>,
    custom_filter: Option<String>,
    hide_done: bool,
    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
    show_only_tagless: bool
}

impl CustomFilterInfo {
    pub fn new() -> Self {
        Self {
            store: None,
            custom_filter: None,
            hide_done: false,
            show_only_tagless: false,
            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray
        }
    }
}

impl Component for CustomFilterInfo {
    fn name() -> ComponentName {
        ComponentName::CurrentFilterInfo
    }

    fn register_store(&mut self, store: Rc<RefCell<Store>>) -> Result<()> {
        self.store = Some(store);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Rc<RefCell<Config>>) -> Result<()> {
        let user_data = &config.try_borrow_mut()?.user_data;
        self.text_color = user_data.text;
        self.border_focused_color = user_data.border_focused;
        self.border_default_color = user_data.border_default;
        Ok(())
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, _tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::CustomFilterUpdated => {
                let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
                let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
                self.custom_filter = taskmanager.get_custom_filter();
            }

            Action::HideAllStatus(status) => self.hide_done = *status,
            Action::ShowOnlyTaglessStatus(status) => self.show_only_tagless = *status,
            _ => {}
        }
        Ok(vec![])
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, is_focused: bool) {
        let style = match is_focused {
            true => Style::default().fg(self.border_focused_color),
            false => Style::default().fg(self.border_default_color)
        };

        let block = Block::bordered()
            .style(Style::new()).title("additional filter").border_style(style);


        let mut lines: Vec<Line> = vec![];
        lines.push(Line::from(""));
        lines.push(Line::from(self.custom_filter.clone().unwrap_or_else(|| "o no status filter active".to_string())));
        lines.push(Line::from(""));

        // add hide_done line
        let mut line = Line::from("o hide completed tasks: ");
        let span = match self.hide_done {
            true => Span::styled("[x]", Style::new().fg(Color::Green)),
            false => Span::from("[ ]"),
        };
        line.push_span(span);
        lines.push(line);
        lines.push(Line::from(""));

        // add show_only_tagless line
        let mut line = Line::from("o show only tasks without tags: ");
        let span = match self.show_only_tagless {
            true => Span::styled("[x]", Style::new().fg(Color::Green)),
            false => Span::from("[ ]"),
        };
        line.push_span(span);
        lines.push(line);

        let paragraph = Paragraph::new(lines).block(block).style(Style::default().fg(self.text_color));
        frame.render_widget(paragraph, area);
    }
}
