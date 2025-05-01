use std::{cell::RefCell, rc::Rc};
use chrono::NaiveDateTime;
use ratatui::{
    prelude::*, widgets::{
        Block, Paragraph
    }
};
use anyhow::Result;
use carpem_core::utils::task::Task;


use crate::{
    action::{Action, RoutedAction}, app::Store, components::Component, config::Config, tui::Tui
};

use super::ComponentName;

pub struct CurrentTaskInfo {
    store: Option<Rc<RefCell<Store>>>,
    current_task: Option<Task>,
    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
}

impl CurrentTaskInfo {
    pub fn new() -> Self {
        Self {
            store: None,
            current_task: None,
            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
        }
    }

    pub fn update_current_task(&mut self, task: Option<Task>) {
        self.current_task = task;
    }

    fn format_date(&self, date: Option<NaiveDateTime>) -> String {
        match date {
            Some(d) => d.format("%d.%m.%Y %H:%M %A").to_string(),
            None => "none".to_string()
        }
    }
}

impl Component for CurrentTaskInfo {
    fn name() -> ComponentName {
        ComponentName::CurrentTaskInfo
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
            Action::CurrentTaskUpdated(task, _) => {
                self.update_current_task(task.clone());
            }
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
            .style(Style::new()).title("task info").border_style(style);

        let mut text = vec![];

        match &self.current_task {
            Some(task) => {
                text.push(Line::from(format!("task title: {}", task.title)).bold());

                // due date
                text.push(Line::from(format!("due date: {}", self.format_date(task.due_date))));

                // reminder date
                text.push(Line::from(format!("reminder date: {}", self.format_date(task.reminder_date))));

                // timer start
                text.push(Line::from(format!(
                        "started at: {}",
                        task.timer_start
                        .map_or("not started".to_string(), |t| t.format("%d.%m.%Y %H:%M:%S").to_string())
                )));

                // time taken
                text.push(Line::from(format!( "time taken: {}", task.time_taken .map_or("none".to_string(), |t| {
                    let total_seconds = t.num_seconds();
                    let hours = total_seconds / 3600;
                    let minutes = (total_seconds % 3600) / 60;
                    let seconds = total_seconds % 60;
                    let time = format!("{:02}:{:02}:{:02} (HH:MM:SS)", hours, minutes, seconds);
                    time 
                }))));

                text.push(Line::from(""));

                // description
                let descriptions: Vec<&str> = task.description.split("\n").collect();
                text.push(Line::from("description:").bold());
                for &d in descriptions.iter() {
                    text.push(Line::from(d));
                }
            }
            None => ()
        }
        let paragraph = Paragraph::new(text).block(block).style(Style::default().fg(self.text_color));
        frame.render_widget(paragraph, area);
    }
}
