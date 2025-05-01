use std::cell::RefCell;
use std::rc::Rc;
use anyhow::{Context, Result};

use chrono::{NaiveDateTime, NaiveTime};
use ratatui::widgets::StatefulWidgetRef;
use serde::{Deserialize, Serialize};
use ratatui::{
    layout::Constraint, prelude::*, widgets::Block
};
use carpem_core::utils::event::{RecurrenceDeletion, RecurrenceInterval};
use crate::action::{ActionTarget, RoutedAction};
use crate::config::Config;
use crate::utils::user_interaction::fzf_select_indices;
use crate::utils::{user_interaction,
    table_widget::{
        Row,Table,TableState
    }
};

use crate::{
    action::Action, app::Store, components::Component, tui::Tui
};

use super::ComponentName;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceSelection {
    None,
    EveryNDays,
    EveryWeekday,
    EveryStartOfMonth,
    EveryEndOfMonth,
    EveryYear,
    EveryNWeeks
}


pub struct CalendarInfo<'a> {
    store: Option<Rc<RefCell<Store>>>,
    current_date: Option<NaiveDateTime>,
    table: Table<'a>,
    state: TableState,
    event_indices: Vec<usize>,

    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
    table_header_color: Color,
    table_highlighted_fg: Color,
    table_highlighted_bg: Color
}

impl<'a> CalendarInfo<'a> {
    pub fn new() -> Self {
        let cal_info = Self {
            store: None,
            current_date: None,
            table: Table::default(),
            state: TableState::default(),
            event_indices: vec![], 

            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
            table_header_color: Color::White,
            table_highlighted_fg: Color::Black,
            table_highlighted_bg: Color::Yellow,
        };

        cal_info
    }

    pub fn select_event_index(&mut self, event_index: usize) {
        for (i, current_event_index) in self.event_indices.iter().enumerate() {
            if *current_event_index == event_index {
                self.state.select(Some(i));
                break;
            }
        }
    }

    pub fn select_event_index_by_id(&mut self, id: Option<i64>) -> Result<()> {
        let mut index_to_select = None;
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;

            for event_index in self.event_indices.iter() {
                if eventmanager.get_events()[*event_index].get_id() == id {
                    index_to_select = Some(*event_index);
                    break;
                }
            }
        }

        match index_to_select {
            Some(index) => self.select_event_index(index),
            None => ()
        }
        
        Ok(())
    }


    pub fn next(&mut self) {
        self.state.select_next();
    }

    pub fn prev(&mut self) {
        self.state.select_previous();
    }

    fn update_table(&mut self) -> Result<()> {
        let Some(current_date) = self.current_date else { return Ok(()); };
        let mut my_rows = vec![];

        let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
        let eventmanager = &mut store.try_borrow_mut()?.eventmanager;

        let date_format: &str = "%H:%M";
        self.event_indices.clear();
        let result = eventmanager.get_events_for_date(&current_date)?;

        for (i, event, start_date) in result.iter() {
            let mut entries = vec![];
            let mut from = start_date.format(date_format).to_string();
            let to = match event.duration {
                Some(duration) => {
                    let end_date = (*start_date + duration).format(date_format).to_string();
                    format!("-{}", end_date)
                }
                None => "".to_string()
            };
            from.push_str(&to);
            entries.push(from);
            entries.push(event.title.clone());
            let recurrence_string = match &event.recurrence_interval {
                Some(rec) => rec.to_string(),
                None => "".to_string()
            };
            entries.push(recurrence_string);

            let current_row = Row::new(entries);
            my_rows.push(current_row);
            self.event_indices.push(*i);
        }

        let widths = [
            Constraint::Length(34),
            Constraint::Fill(2),
            Constraint::Fill(1)
        ];

        self.table = Table::new(my_rows, widths)
            .header(
                Row::new(["date", "title", "recurring"])
                .style(Style::default().bold().fg(self.table_header_color))
                .bottom_margin(1))
                .column_spacing(1)
                .row_highlight_style(Style::new().bg(self.table_highlighted_bg).fg(self.table_highlighted_fg).bold())
                .show_relative_line_number(false);

        Ok(())
    }

    fn get_current_event_cloned(&self) -> Result<Option<(usize, carpem_core::utils::event::Event)>> {
        if let Some(index) = self.state.selected() {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            let event_index = self.event_indices[index];
            return Ok(Some((event_index, eventmanager.get_events()[event_index].clone())));
        }
        Ok(None)
    }

    pub fn update_event(&mut self, tui: &mut Tui) -> Result<()>{
        let (index, mut current_event) = match self.get_current_event_cloned()? {
            Some(event) => event, 
            None => return Ok(())
        };
        let file_contents = format!("[title]\n{}\n\n[description]\n{}", 
            current_event.title, current_event.description);
        let reader = user_interaction::get_input_from_editor(file_contents, tui)?;
        let event = user_interaction::event_from_reader(reader); 
        current_event.title = event.title.clone();
        current_event.description = event.description.clone();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            eventmanager.update_event(current_event, &Some(index))?;
        }
        self.update_table()?;
        Ok(())
    }

    pub fn edit_date(&mut self, tui: &mut Tui) -> Result<()> {
        let Some((index, mut event)) = self.get_current_event_cloned()? else { return Ok(()); };

        let date = user_interaction::get_date_input(tui, event.date)?;
        event.date = date;
        event.update_date()?;
        self.current_date = Some(event.date.clone());
        let event_id = event.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            eventmanager.update_event(event, &Some(index))?;
        }
        self.update_table()?;
        self.select_event_index_by_id(event_id)?;
        Ok(())
    }

    pub fn delete_event(&mut self, tui: &mut Tui) -> Result<()> {
        let Some((index, event)) = self.get_current_event_cloned()? else { return Ok(()); };
        let Some(current_date) = self.current_date else { return Ok(()); };

        let deletion_option = if event.recurrence_interval.is_some() {
            // ask the user if he wants to delete the whole event or just some specific dates
            // current only, all from here, delete all

            let time = NaiveTime::from_hms_opt(0, 0, 0)
                .with_context(|| "could not create NaiveTime!")?;
            let date = NaiveDateTime::new(current_date.date(), time);
            let options = vec![ 
                serde_json::to_string(&RecurrenceDeletion::All)?,
                serde_json::to_string(&RecurrenceDeletion::FromHere(date))?,
                serde_json::to_string(&RecurrenceDeletion::ThisOnly(date))?,
            ];
            let fzf_result = user_interaction::fzf_select(tui, &options)?;

            let deletion_option: RecurrenceDeletion = match fzf_result.selected.len() {
                n if n >= 1 => { serde_json::from_str(&fzf_result.selected[0])? }
                _ => {
                    return Ok(());
                }
            };
            Some(deletion_option)
        } else { None };

        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            eventmanager.delete_event(index, deletion_option)?;
        }
        self.update_table()?;
        Ok(())
    }

    pub fn edit_recurrence(&mut self, tui: &mut Tui) -> Result<()> {
        let Some((index, mut event)) = self.get_current_event_cloned()? else { return Ok(()); };

        let options = vec![
            serde_json::to_string(&RecurrenceSelection::None)?,
            serde_json::to_string(&RecurrenceSelection::EveryNDays)?,
            serde_json::to_string(&RecurrenceSelection::EveryWeekday)?,
            serde_json::to_string(&RecurrenceSelection::EveryStartOfMonth)?,
            serde_json::to_string(&RecurrenceSelection::EveryEndOfMonth)?,
            serde_json::to_string(&RecurrenceSelection::EveryYear)?,
            serde_json::to_string(&RecurrenceSelection::EveryNWeeks)?,
        ];
        let fzf_result = user_interaction::fzf_select(tui, &options)?;

        let recurrence_selection: RecurrenceSelection = match fzf_result.selected.len() {
            n if n >= 1 => { serde_json::from_str(&fzf_result.selected[0])? }
            _ => {
                return Ok(());
            }
        };

        let recurrence_interval: Option<RecurrenceInterval> = match recurrence_selection {
            RecurrenceSelection::None => None,
            RecurrenceSelection::EveryNDays => {
                match user_interaction::get_user_number(tui, &event)? {
                    Some(num) if num > 0 => Some(RecurrenceInterval::EveryNDays(num)),
                    _ => return Ok(())
                } 
            }

            RecurrenceSelection::EveryWeekday => {
                // choose weekday
                let weekday_options = vec![
                    chrono::Weekday::Sun,
                    chrono::Weekday::Sat,
                    chrono::Weekday::Fri,
                    chrono::Weekday::Thu,
                    chrono::Weekday::Wed,
                    chrono::Weekday::Tue,
                    chrono::Weekday::Mon,
                ];

                let mut fzf_options = vec![];
                for (i, weekday) in weekday_options.iter().enumerate() {
                    let mut s = i.to_string();
                    s.push_str(" ");
                    let option_string = serde_json::to_string(weekday)?;
                    s.push_str(&option_string);
                    fzf_options.push(s);
                }

                let indices = fzf_select_indices(tui, &fzf_options)?;
                let mut weekdays = vec![];
                for &i in indices.iter() {
                    let weekday = weekday_options[i];
                    weekdays.push(weekday);
                }

                match weekdays.len() > 0 {
                    true => Some(RecurrenceInterval::EveryWeekday(weekdays)),
                    false => return Ok(())
                }
            }

            RecurrenceSelection::EveryYear =>  Some(RecurrenceInterval::EveryYear), 
            RecurrenceSelection::EveryStartOfMonth => Some(RecurrenceInterval::EveryStartOfMonth),
            RecurrenceSelection::EveryEndOfMonth => Some(RecurrenceInterval::EveryEndOfMonth),
            RecurrenceSelection::EveryNWeeks => {
                match user_interaction::get_user_number(tui, &event)? {
                    Some(num) if num > 0 => Some(RecurrenceInterval::EveryNWeeks(num)),
                    _ => return Ok(())
                } 
            }
        };

        event.recurrence_interval = recurrence_interval;
        event.update_date()?;
        self.current_date = Some(event.date);
        let event_id = event.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            eventmanager.update_event(event, &Some(index))?;
        }
        self.update_table()?;
        self.select_event_index_by_id(event_id)?;
        Ok(())
    }

    pub fn edit_duration(&mut self, tui: &mut Tui) -> Result<()> {
        let Some((index, mut event)) = self.get_current_event_cloned()? else { return Ok(()); };
        let result = user_interaction::get_event_duration_from_user(tui, &event)?;

        if result.is_some() {
            event.duration = result;
        }

        {
            let store = self.store.as_ref().with_context(|| "could not access store to get eventmanager!")?;
            let eventmanager = &mut store.try_borrow_mut()?.eventmanager;
            eventmanager.update_event(event, &Some(index))?;
        }
        self.update_table()?;
        Ok(())
    }
}

impl<'a> Component for CalendarInfo<'a> {
    fn name() -> ComponentName {
        ComponentName::CurrentDayList
    }

    fn register_store(&mut self, store: Rc<RefCell<Store>>) -> Result<()> {
        self.store = Some(store);
        Ok(())
    }

    fn init(&mut self, _area: Size) -> Result<Vec<RoutedAction>> {
        self.update_table()?;
        Ok(vec![])
    }

    fn register_config_handler(&mut self, config: Rc<RefCell<Config>>) -> Result<()> {
        let user_data = &config.try_borrow_mut()?.user_data;
        self.text_color = user_data.text;
        self.border_focused_color = user_data.border_focused;
        self.border_default_color = user_data.border_default;
        self.table_header_color = user_data.table_header_color;
        self.table_highlighted_fg = user_data.table_highlighted_fg;
        self.table_highlighted_bg = user_data.table_highlighted_bg;
        Ok(())
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::CurrentDateUpdated(date) => {
                self.current_date = Some(*date);
                self.update_table()?;
            }

            Action::SelectEventIndex(index) => {
                self.select_event_index(*index);
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ])
            }

            Action::UpdateCurrentDay => {
                self.update_table()?;
            }

            Action::NextEvent => {
                self.next();
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::PrevEvent => {
                self.prev();  
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::EditEvent => {
                self.update_event(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::EditDurationTime => {
                self.edit_duration(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::EditRecurrence => {
                self.edit_recurrence(tui)?;
                if let Some(current_date) = self.current_date {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::CurrentDateUpdated(current_date),
                            target: ActionTarget::All
                        },
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        }
                    ]);
                }
            }

            Action::EditDate => {
                self.edit_date(tui)?;
                if let Some(current_date) = self.current_date {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::CurrentDateUpdated(current_date),
                            target: ActionTarget::All
                        },
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        }
                    ]);
                }
            }

            Action::DeleteEvent => {
                self.delete_event(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
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
            .style(Style::new()).title("events for selected date").border_style(style);

        self.table.block = Some(block); 
        self.table.render_ref(area, frame.buffer_mut(), &mut self.state);
    }
}
