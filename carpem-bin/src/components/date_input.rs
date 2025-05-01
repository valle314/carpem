use std::collections::HashMap;
use chrono::{Local, NaiveDateTime};
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::prelude::*;
use crate::{
    action::{Action, ActionTarget, RoutedAction}, components::Component, tui::Tui,
    utils::user_interaction
};
use date_parser;
use super::ComponentName;

pub struct DateInput {
    start_date: NaiveDateTime,
    result_date: NaiveDateTime,
    input: tui_input::Input,
    parsed_hashmap: HashMap<(String, NaiveDateTime), NaiveDateTime>,
}

impl DateInput {
    pub fn new() -> Self {
        Self { 
            start_date: Local::now().naive_local(),
            result_date: Local::now().naive_local(),
            input: tui_input::Input::default(),
            parsed_hashmap: HashMap::new(),
        }
    }

    fn handle_key_input(&mut self, keycode: KeyCode) {
        match keycode {
            keycode => {
                if let Some(req) = user_interaction::tui_input_handle_event(keycode) {
                    self.input.handle(req);
                    let string_to_parse = self.input.value().split_whitespace();
                    let mut parsed_date = self.start_date.clone();
                    for s in string_to_parse {
                        if let Some(hashmap_res) = self.parsed_hashmap.get(&(s.to_string(), self.start_date)) {
                            parsed_date = *hashmap_res;
                        } else {
                            if let Some(new_date) = date_parser::parser::parse_date(s, parsed_date) {
                                parsed_date = new_date;
                                self.parsed_hashmap.insert((s.to_string(),parsed_date), new_date);
                            }
                        }
                    }
                    self.result_date = parsed_date;
                }
            }
        }
    }
}

impl Component for DateInput {
    fn name() -> ComponentName {
        ComponentName::DateInput
    }

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent, handled: &mut bool, _tui: &mut Tui) -> Result<Vec<RoutedAction>> {
        *handled = true;
        self.handle_key_input(key.code);
        match key.code {
            KeyCode::Enter => {
                self.input.reset();
                self.parsed_hashmap.clear();
                *handled = true;
                return Ok( vec![
                    RoutedAction {
                        action: Action::QuitPopup,
                        target: ActionTarget::App
                    },
                    RoutedAction {
                        action: Action::CurrentDateUpdated(self.result_date),
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ])
            }

            KeyCode::Esc => {
                self.input.reset();
                self.parsed_hashmap.clear();
                *handled = true;
                return Ok( vec![
                    RoutedAction {
                        action: Action::QuitPopup,
                        target: ActionTarget::App
                    },
                    RoutedAction {
                        action: Action::CurrentDateUpdated(self.start_date),
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ])
            }

            _ => ()
        }

        Ok(vec![
            RoutedAction {
                action: Action::CurrentDateUpdated(self.result_date),
                target: ActionTarget::All
            },
            RoutedAction {
                action: Action::Render,
                target: ActionTarget::App
            }
        ])
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, _tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::CurrentDateUpdated(date) => {
                if self.input.value().is_empty() {
                    self.start_date = *date;
                }
            }
            _ => {}
        }
        Ok(vec![])
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, _is_focused: bool) {
        user_interaction::render_tui_input(frame, area, &self.input);
    }
}
