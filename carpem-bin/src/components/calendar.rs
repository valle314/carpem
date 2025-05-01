use std::{cell::RefCell, rc::Rc};
use anyhow::Result;

use anyhow::Context;
use chrono::{Days, Local};
use ratatui::{
    prelude::*, widgets::Block
    
};
use crate::config::Config;
use crate::{action::{ActionTarget, RoutedAction}, 
    utils::calendar_widget::{self, CalendarEventStore, Monthly}};
use crate::utils::user_interaction;

use crate::{
    action::Action, app::Store, components::Component, tui::Tui
};

use super::ComponentName;

pub struct Calendar<'a> {
    store: Option<Rc<RefCell<Store>>>,
    calendar_widget: Monthly<'a, CalendarEventStore>,

    border_focused_color: Color,
    border_default_color: Color,
}

impl<'a> Calendar<'a> {
    pub fn new() -> Self {
        let current_date = Local::now().naive_local();
        let calendar_widget = calendar_widget::Monthly::new(current_date, calendar_widget::CalendarEventStore::default()) 
            .show_weekdays_header(Style::default().fg(Color::Reset))
            .show_month_header(Style::default());

        Self {
            store: None,
            calendar_widget,

            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
        }
    }

    pub fn go_to_current_date(&mut self) {
        self.calendar_widget.display_date = Local::now().naive_local();
    }

    pub fn next_day(&mut self) -> Result<()> {
        let display_date = self.calendar_widget.display_date;
        self.calendar_widget.display_date = display_date.checked_add_days(Days::new(1)).unwrap_or_else( || display_date);
        Ok(())
    }

    pub fn prev_day(&mut self) -> Result<()> {
        let display_date = self.calendar_widget.display_date;
        self.calendar_widget.display_date = display_date.checked_sub_days(Days::new(1)).unwrap_or_else( || display_date);
        Ok(())
    }

    pub fn next_week(&mut self) -> Result<()> {
        let display_date = self.calendar_widget.display_date;
        self.calendar_widget.display_date = display_date.checked_add_days(Days::new(7)).unwrap_or_else( || display_date);
        Ok(())
    }

    pub fn prev_week(&mut self) -> Result<()> {
        let display_date = self.calendar_widget.display_date;
        self.calendar_widget.display_date = display_date.checked_sub_days(Days::new(7)).unwrap_or_else( || display_date);
        Ok(())
    }

    pub fn find_event(&mut self, tui: &mut Tui) -> Result<Option<usize>> {
        let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
        let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
        let mut options: Vec<String> = vec![];

        let events = eventmanager.get_events();
        for (i, event) in events.iter().enumerate() {
            let mut s = i.to_string();
            s.push_str(" ");
            s.push_str(&event.title);
            s.push_str("\n");
            s.push_str(&event.description);
            options.push(s);
        }
        let indices = user_interaction::fzf_select_indices(tui, &options)?;
        let index = if indices.len() != 0 { indices[0] } else { return Ok(None); };
        self.calendar_widget.display_date = events[index].date;
        Ok(Some(index))
    }

    pub fn add_event(&mut self, tui: &mut Tui) -> Result<Option<usize>> {
        let file_contents = "[title]\n\n[description]".to_string();
        let reader = user_interaction::get_input_from_editor(file_contents, tui)?;
        let mut event = user_interaction::event_from_reader(reader);
        event.date = self.calendar_widget.display_date;
        if event.title.trim().is_empty() { return Ok(None); } // no title given

        let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
        let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
        Ok(Some(eventmanager.add_event(event)?))
    }

    fn get_updated_date_actions(&self) -> Result<Vec<RoutedAction>> {
        Ok(vec![
            RoutedAction {
                action: Action::CurrentDateUpdated(self.calendar_widget.display_date),
                target: ActionTarget::All
            },
            RoutedAction {
                action: Action::Render,
                target: ActionTarget::App
            }
        ])
    }
}

impl<'a> Component for Calendar<'a> {
    fn name() -> ComponentName {
        ComponentName::Calendar
    }

    fn register_store(&mut self, store: Rc<RefCell<Store>>) -> Result<()> {
        self.store = Some(store);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Rc<RefCell<Config>>) -> Result<()> {
        let user_data = &config.try_borrow_mut()?.user_data;
        self.border_focused_color = user_data.border_focused;
        self.border_default_color = user_data.border_default;

        self.calendar_widget.default_style = Style::new().fg(user_data.text);
        self.calendar_widget.show_month = Some(Style::new().fg(user_data.text));
        self.calendar_widget.show_weekday = Some(Style::new().fg(user_data.text));
        self.calendar_widget.iso_week_style = Style::new()
            .fg(user_data.calendar_iso_week_fg)
            .bg(user_data.calendar_iso_week_bg);

        self.calendar_widget.events.display_date_style = Style::default()
            .fg(user_data.calendar_date_highlight_fg)
            .bg(user_data.calendar_date_highlight_bg);
        Ok(())
    }

    fn init(&mut self, _area: Size) -> Result<Vec<RoutedAction>> {
        Ok(vec![
            RoutedAction {
                action: Action::CurrentDateUpdated(self.calendar_widget.display_date),
                target: ActionTarget::All
            },
        ])
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, tui: &mut Tui) -> Result<Vec<RoutedAction>> {
        match action {
            Action::GoToToday => {
                self.go_to_current_date();
                return self.get_updated_date_actions();
            }

            Action::GoToPrevDay => {
                self.prev_day()?;
                return self.get_updated_date_actions();
            }

            Action::GoToNextDay => {
                self.next_day()?;
                return self.get_updated_date_actions();
            }

            Action::GoToPrevWeek => {
                self.prev_week()?;
                return self.get_updated_date_actions();
            }

            Action::GoToNextWeek => {
                self.next_week()?;
                return self.get_updated_date_actions();
            }

            Action::FindDate => {
                return Ok(vec![
                    RoutedAction {
                        action: Action::Popup(ComponentName::DateInput),
                        target: ActionTarget::App
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::FZFFindEvent => {
                if let Some(index) = self.find_event(tui)? {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::CurrentDateUpdated(self.calendar_widget.display_date),
                            target: ActionTarget::All
                        },
                        RoutedAction {
                            action: Action::SelectEventIndex(index),
                            target: ActionTarget::All
                        },
                        RoutedAction { // switch focus to the found event
                            action: Action::FocusDown,
                            target: ActionTarget::App
                        },
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        }
                    ]);
                }
            }

            Action::AddEvent => {
                if let Some(index) = self.add_event(tui)? {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::UpdateCurrentDay,
                            target: ActionTarget::Specific(vec![ComponentName::CurrentDayList])
                        },
                        RoutedAction {
                            action: Action::SelectEventIndex(index),
                            target: ActionTarget::All
                        },
                        RoutedAction { // switch focus to the found event
                            action: Action::FocusDown,
                            target: ActionTarget::App
                        },
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        }
                    ]);
                }
            }

            Action::CurrentDateUpdated(date) => {
                self.calendar_widget.display_date = *date;
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
            .style(Style::new()).title("calendar").border_style(style);

        self.calendar_widget.block = Some(block);
        frame.render_widget_ref(&self.calendar_widget, area);
    }
}
