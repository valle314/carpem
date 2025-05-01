use std::cell::RefCell;
use std::rc::Rc;
use anyhow::{Context, Result};
use ratatui::widgets::StatefulWidgetRef;
use carpem_core::utils::task::TaskOrder;
use chrono::{Duration, Local, Utc};
use ratatui::{
    prelude::*, 
    widgets::Block,
    style::Color
};

use crate::utils::table_widget::{TableState, Table, Row};
use carpem_core::utils::{status::Status, task::Task};

use crate::action::{ActionTarget, RoutedAction};
use crate::config::Config;
use crate::tui::Tui;
use crate::{
    components::Component,
    action::Action,
    app::Store,
    utils::user_interaction,

};

use super::ComponentName;

pub struct TaskComponent<'a> {
    table: Table<'a>,
    state: TableState,
    store: Option<Rc<RefCell<Store>>>,
    config: Option<Rc<RefCell<Config>>>,
    rows_len: usize,
    current_status_filter: Vec<Status>,

    border_focused_color: Color,
    border_default_color: Color,
}

impl<'a> TaskComponent<'a> {
    pub fn new() -> Self {
        let table_state = TableState::default();
        let mut table = Table::default();
        table.show_relative_line_number = true;

        let task_table = Self {
            table,
            state: table_state,
            store: None,
            config: None,
            rows_len: 0,
            current_status_filter: vec![],

            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
        };

        task_table
    }

    // ------------------------ update model ------------------------
    fn update_table(&mut self) -> Result<()> {
        let mut my_rows = vec![];

        // update the current selection first
        self.rows_len = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            store.try_borrow()?.taskmanager.get_tasks().len()
        };

        match self.state.selected() {
            Some(index) => if index >= self.rows_len { self.select_last(); },
            None => ()
        }
        
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &store.try_borrow_mut()?.taskmanager;
            let tasks_to_show = taskmanager.get_tasks();

            for task in tasks_to_show.iter() {
                let mut entries = vec![];
                entries.push(task.title.clone());
                entries.push(serde_json::to_string(&task.status)?);
                entries.push(task.severity.to_string());

                let mut row_style = Style::default();
                if let Some(config) = &self.config {
                    let user_data = &config.try_borrow_mut()?.user_data;
                    let (fg, bg) = match (&task.status, task.timer_start.is_some()) { 
                        (_, true) => (user_data.is_time_tracked_fg, user_data.is_time_tracked_bg),
                        (Status::Backlog, false) => (user_data.status_backlog_fg, user_data.status_backlog_bg),
                        (Status::Pending, false) => (user_data.status_pending_fg, user_data.status_pending_bg),
                        (Status::InProgress, false) => (user_data.status_in_progress_fg, user_data.status_in_progress_bg),
                        (Status::Testing, false) => (user_data.status_testing_fg, user_data.status_testing_bg),
                        (Status::Done, false) => (user_data.status_done_fg, user_data.status_done_bg),
                    };

                    row_style = row_style.bg(bg).fg(fg);
                }

                let current_row = Row::new(entries).style(row_style);
                my_rows.push(current_row);
            }
        }

        let widths = [
            Constraint::Fill(1),
            Constraint::Length(12),
            Constraint::Length(8)
        ];

        let (header_color, highlight_fg, highlight_bg) = if let Some(config) = &self.config {
            let user_data = &config.try_borrow_mut()?.user_data;
            (user_data.table_header_color, user_data.table_highlighted_fg, user_data.table_highlighted_bg)
        } else {
            (Color::Reset, Color::LightYellow, Color::Gray)
        };

        self.table = Table::new(my_rows, widths)
            .header(
                Row::new(["title", "status", "severity"])
                .style(Style::default().bold().fg(header_color))
                .bottom_margin(1))
                .column_spacing(1)
                .row_highlight_style(Style::new().bg(highlight_bg).fg(highlight_fg).bold())
                .show_relative_line_number(true);

        Ok(())
    }

    // ------------------------ state related ------------------------
    pub fn current_row(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn next(&mut self, amount: usize) {
        if self.rows_len > 0 {
            let mut new_selection = match self.state.selected() {
                Some(row) => row.saturating_add(amount),
                None => amount.saturating_sub(1)
            };
            if new_selection >= self.rows_len {
                new_selection = self.rows_len.saturating_sub(1);
            }
            self.state.select(Some(new_selection));
        }
    }

    pub fn prev(&mut self, amount: usize) {
        if self.rows_len > 0 {
            let new_selection = match self.state.selected() {
                Some(row) => row.saturating_sub(amount),
                None => self.rows_len.saturating_sub(amount)
            };
            self.state.select(Some(new_selection));
        }
    }

    pub fn select_first(&mut self) {
        if self.rows_len > 0 {
            self.state.select(Some(0));
        }
    }

    pub fn select_last(&mut self) {
        if self.rows_len > 0 {
            self.state.select(Some(self.rows_len.saturating_sub(1)));
        }
    }

    fn select_task_by_id(&mut self, id: Option<i64>) -> Result<()> {
        if id.is_none() { return Ok(()); };
        let mut index_to_select = None;
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            for (i, task) in taskmanager.get_tasks().iter().enumerate() {
                if task.get_id() == id {
                    index_to_select = Some(i);
                    break;
                }
            }
        }
        self.state.select(index_to_select);
        Ok(())
    }

    fn update_and_select_if_possible(&mut self, task_id: Option<i64>) -> Result<()> {
        self.update_table()?;
        self.select_task_by_id(task_id)?;
        Ok(())
    }

    // ------------------------ tasks crud ------------------------
    pub fn add_task(&mut self, tui: &mut Tui) -> Result<()> {
        let file_contents = "[title]\n\n[description]\n\n[severity]\n0".to_string();
        let reader = user_interaction::get_input_from_editor(file_contents, tui)?;
        let mut task = user_interaction::task_from_reader(reader);
        if task.title.trim().is_empty() { return Ok(()); }

        let (reselect_task, task_id)= {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let len_before = taskmanager.get_tasks().len();
            task.tags = taskmanager.get_tag_filter().to_vec();
            if self.current_status_filter.len() == 1 {
                task.status = self.current_status_filter[0].clone();
            } else {
                task.status = Status::Backlog;
            }

            let task_id = taskmanager.add_task(task)?;
            (taskmanager.get_tasks().len() > len_before, task_id)
        };

        self.update_and_select_if_possible({
            if reselect_task { task_id } else { None } 
        })?;

        Ok(())
    }

    pub fn update_task(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return  Ok(()); };
        let file_contents = format!("[title]\n{}\n\n[description]\n{}\n\n[severity]\n{}", 
            current_task.title, current_task.description, current_task.severity.to_string());
        let reader = user_interaction::get_input_from_editor(file_contents, tui)?;
        let task = user_interaction::task_from_reader(reader);
        current_task.title = task.title.clone();
        current_task.description = task.description.clone();
        current_task.severity = task.severity;
        let task_id = current_task.get_id();

        let reselect_task = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let len_before = taskmanager.get_tasks().len();

            taskmanager.update_task(current_task, &self.current_row())?;
            taskmanager.get_tasks().len() == len_before
        };

        self.update_and_select_if_possible({
            if reselect_task { task_id } else { None } 
        })?;

        Ok(())
    }

    pub fn delete_done_tasks(&mut self) -> Result<()> {
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.delete_done_tasks()?;
        }
        self.select_first();
        self.update_table()?;
        Ok(())
    }

    pub fn delete_task(&mut self) -> Result<()> {
        let Some(current_row) = self.current_row() else { return Ok(()); };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.delete_task(current_row)?;
        }

        self.rows_len = self.rows_len.saturating_sub(1);
        if self.rows_len == 0 { 
            self.state.select(None); 
        }

        match (self.state.selected(), self.rows_len) {
            (Some(index), len) => if index >= len { self.state.select_previous(); },
            _ => ()
        }
        self.update_table()?;
        Ok(())
    }

    pub fn start_stop_task(&mut self) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            match current_task.timer_start {
                Some(start_time) => { 
                    // stop timer and add the duration
                    let duration = Utc::now().naive_local() - start_time; 
                    current_task.timer_start = None;
                    let current_time_taken = current_task.time_taken.unwrap_or_else(|| Duration::seconds(0));
                    current_task.time_taken = Some(current_time_taken + duration);
                }
                None => {
                    current_task.timer_start = Some(Utc::now().naive_local());
                }
            }

            taskmanager.update_task(current_task, &self.current_row())?;
        }
        self.update_table()?;
        Ok(())
    }

    pub fn update_status(&mut self, tui: &mut Tui) -> Result<()> {
        let options = vec![
            serde_json::to_string(&Status::Backlog)?,
            serde_json::to_string(&Status::Pending)?,
            serde_json::to_string(&Status::InProgress)?,
            serde_json::to_string(&Status::Testing)?,
            serde_json::to_string(&Status::Done)?,
        ];
        let fzf_result = user_interaction::fzf_select(tui, &options)?;

        let status: Status = match fzf_result.selected.len() {
            n if n >= 1 => { serde_json::from_str(&fzf_result.selected[0])? }
            _ => {
                return Ok(());
            }
        };

        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        current_task.status = status;
        let task_id = current_task.get_id();
        let reselect_task = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let len_before = taskmanager.get_tasks().len();
            taskmanager.update_task(current_task, &self.current_row())?;
            taskmanager.get_tasks().len() == len_before
        };

        self.update_and_select_if_possible({
            if reselect_task { task_id } else { None } 
        })?;

        Ok(())
    }

    pub fn update_reminder_date(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        let start_date = match current_task.due_date {
            Some(d) => d,
            None => Local::now().naive_local()
        };
        let new_reminder_date = user_interaction::get_date_input(tui, start_date)?;
        let task_id = current_task.get_id();
        current_task.reminder_date = Some(new_reminder_date);
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }
        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    pub fn remove_reminder_date(&mut self) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        current_task.reminder_date = None;
        let task_id = current_task.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    pub fn remove_time_taken(&mut self) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };

        current_task.time_taken = None;
        let task_id = current_task.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }
 
    pub fn update_due_date(&mut self, tui: &mut Tui) -> Result<()> {
        let new_due_date = user_interaction::get_date_input(tui, Local::now().naive_local())?;
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        current_task.due_date = Some(new_due_date);
        let task_id = current_task.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    pub fn remove_due_date(&mut self) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        current_task.due_date = None;
        let task_id = current_task.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    pub fn edit_time_taken(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(mut current_task) = self.get_current_task_cloned()? else { return Ok(()); };
        let Some(result) = user_interaction::get_task_duration_from_user(tui, &current_task)? else { return Ok(()); };
        current_task.time_taken = Some(result);
        let task_id = current_task.get_id();
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task, &self.current_row())?;
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    // ------------------------ filter related ------------------------
    pub fn reset_filter(&mut self) -> Result<()> {
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.set_custom_filter(None)?;
            self.current_status_filter = vec![];
        }
        self.update_table()?;
        self.select_first(); 
        Ok(())
    }

    pub fn set_tag_filter(&mut self, tui: &mut Tui) -> Result<()> {
        let mut tags_to_choose = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.get_tags().clone()
        };

        let res = user_interaction::fzf_select_tags(&mut tags_to_choose, tui)?;
        if res.query.is_none() { return Ok(()); };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let mut tag_ids: Vec<i64> = vec![];
            {
                for tag_text in res.selected.iter() {
                    let tag_id = taskmanager.get_id_from_tag_text(&tag_text)?;
                    tag_ids.push(tag_id);
                }
            }
            taskmanager.set_tags_filter(tag_ids)?;
        }
        self.update_table()?;
        self.select_first();
        Ok(())
    }

    pub fn reset_tag_filter(&mut self) -> Result<()> {
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.set_tags_filter(vec![])?;
        }
        self.update_table()?;
        self.select_first();
        Ok(())
    }

    pub fn set_custom_filter(&mut self, tui: &mut Tui) -> Result<()> {
        let options = vec![
            serde_json::to_string(&Status::Backlog)?,
            serde_json::to_string(&Status::Pending)?,
            serde_json::to_string(&Status::InProgress)?,
            serde_json::to_string(&Status::Testing)?,
            serde_json::to_string(&Status::Done)?,
        ];

        let fzf_result = user_interaction::fzf_select(tui, &options)?;
        if fzf_result.selected.len() == 0 { return Ok(()); }

        let mut current_filter_string = "".to_string();
        let mut first_entry: bool = true;

        self.current_status_filter = vec![];
        for selection in fzf_result.selected.iter() {
            let status: Status = serde_json::from_str(&selection)?;
            self.current_status_filter.push(status.clone());
            if first_entry {
                current_filter_string.push_str(&format!("status == '\"{}\"'", status.get_string()));
                first_entry = false;
            } else {
                current_filter_string.push_str(&format!(" OR status == '\"{}\"'", status.get_string()));
            }
        }

        if !first_entry {
            {
                let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
                let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
                taskmanager.set_custom_filter(Some(&current_filter_string))?;
            }
            self.update_table()?;
            self.select_first();
        }
        Ok(())
    }
 
    pub fn order_by(&mut self, tui: &mut Tui) -> Result<()> {
        let task_id = match self.get_current_task_cloned()? {
            Some(task) => task.get_id(),
            None => None
        };

        let options = vec![
            serde_json::to_string(&TaskOrder::SeverityHighToLow)?,
            serde_json::to_string(&TaskOrder::SeverityLowToHigh)?,
            serde_json::to_string(&TaskOrder::ReminderDateHighToLow)?,
            serde_json::to_string(&TaskOrder::ReminderDateLowToHigh)?,
            serde_json::to_string(&TaskOrder::DueDateHighToLow)?,
            serde_json::to_string(&TaskOrder::DueDateLowToHigh)?,
            serde_json::to_string(&TaskOrder::TimeTakenHighToLow)?,
            serde_json::to_string(&TaskOrder::TimeTakenLowToHigh)?,
        ];

        let fzf_result = user_interaction::fzf_select(tui, &options)?;
        let ordering: TaskOrder = match fzf_result.selected.len() {
            n if n >= 1 => { serde_json::from_str(&fzf_result.selected[0])? }
            _ => {
                return Ok(());
            }
        };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.set_ordering(ordering);
        }

        self.update_and_select_if_possible(task_id)?;
        Ok(())
    }

    pub fn toggle_hide_done(&mut self) -> Result<()> {
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.toggle_hide_done()?;
        }

        self.update_table()?;
        self.select_first();
        Ok(())
    }

    pub fn toggle_show_task_without_tags(&mut self) -> Result<()> {
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.toggle_show_task_without_tags()?;
        }

        self.update_table()?;
        self.select_first();
        Ok(())
    }

    // ------------------------ misc ------------------------
    pub fn find_task(&mut self, tui: &mut Tui) -> Result<()> {
        let index_to_select = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let mut options: Vec<String> = vec![];

            let tasks = taskmanager.get_tasks();
            for (i, task) in tasks.iter().enumerate() {
                let mut s = i.to_string();
                s.push_str(" ");
                s.push_str(&task.title);
                s.push_str("\n");
                s.push_str(&task.description);
                options.push(s);
            }
            let indices = user_interaction::fzf_select_indices(tui, &options)?;
            let index = if indices.len() != 0 { indices[0] } else { return Ok(()); };
            Some(index)
        };
        self.state.select(index_to_select);
        self.update_table()?;
        Ok(())
    }

    fn show_tracked_tasks(&mut self, tui: &mut Tui) -> Result<()> {
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
        user_interaction::show_tacked_task_popup(tui, taskmanager)?;
        Ok(())
    }

    // ------------------------ util functions ------------------------
    fn get_current_task_cloned(&self) -> Result<Option<Task>> {
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
        let task = taskmanager.get_current_task(&self.current_row());
        return Ok(task.cloned());
    }

    // ------------------------ helper functions for sending actions ------------------------
    fn get_show_tagless_action(&self) -> Result<RoutedAction> {
        let status = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.get_show_tagless_status()
        };

        Ok(RoutedAction { 
            action: Action::ShowOnlyTaglessStatus(status), 
            target: ActionTarget::Specific(vec![ComponentName::CurrentFilterInfo]) 
        })
    }

    fn get_updated_table_actions(&self) -> Result<Vec<RoutedAction>> {
        return Ok(vec![
            RoutedAction {
                action: self.get_task_updated_action()?,
                target: ActionTarget::All
            },
            RoutedAction {
                action: Action::Render,
                target: ActionTarget::App
            },
        ]);
    }

    fn get_task_updated_action(&self) -> Result<Action> {
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
        let index = self.current_row();
        let task = taskmanager.get_current_task(&index);
        Ok(Action::CurrentTaskUpdated(task.cloned(), index))
    }

    fn get_hide_done_action(&self) -> Result<RoutedAction> {
        let status = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.get_hide_done_status()
        };

        Ok(RoutedAction { 
            action: Action::HideAllStatus(status), 
            target: ActionTarget::Specific(vec![ComponentName::CurrentFilterInfo]) 
        })
    }
}

impl Component for TaskComponent<'_> {
    fn name() -> ComponentName {
        ComponentName::TaskList
    }

    fn register_config_handler(&mut self, config: Rc<RefCell<Config>>) -> Result<()> {
        {
            let user_data = &config.try_borrow_mut()?.user_data;
            self.border_focused_color = user_data.border_focused;
            self.border_default_color = user_data.border_default;
        }
        self.config = Some(config);
        Ok(())
    }

    fn register_store(&mut self, store: Rc<RefCell<Store>>) -> Result<()> {
        self.store = Some(store);
        Ok(())
    }

    fn init(&mut self, _area: Size) -> Result<Vec<RoutedAction>> {
        self.update_table()?;
        Ok(vec![])
    }

    fn update(&mut self, prefix_number: usize, action: &Action, tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::DeleteTask => {
                self.delete_task()?;
                return self.get_updated_table_actions();
            }

            Action::DeleteAllDoneTasks => {
                self.delete_done_tasks()?;
                return self.get_updated_table_actions();
            }

            Action::AddTask => {
                self.add_task(tui)?;
                return self.get_updated_table_actions();
            }
            Action::NextTask => {
                self.next(if prefix_number != 0 { prefix_number } else { 1 });
                return self.get_updated_table_actions();
            }

            Action::PrevTask => {
                self.prev(if prefix_number != 0 { prefix_number } else { 1 });
                return self.get_updated_table_actions();
            }

            Action::SelectFirst => {
                self.select_first();
                return self.get_updated_table_actions();
            }

            Action::SelectLast => {
                self.select_last();
                return self.get_updated_table_actions();
            }

            Action::ResetDueDate => {
                self.remove_due_date()?;
                return self.get_updated_table_actions();
            }

            Action::ResetReminderDate => {
                self.remove_reminder_date()?;
                return self.get_updated_table_actions();
            }

            Action::ResetTimeTaken => {
                self.remove_time_taken()?;
                return self.get_updated_table_actions();
            }

            Action::UpdateTaskTable => {
                self.update_table()?;
                self.select_first();
                return self.get_updated_table_actions();
            }

            Action::ResetStatusFilter => {
                self.reset_filter()?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::CustomFilterUpdated, 
                        target: ActionTarget::Specific(vec![ComponentName::CurrentFilterInfo])
                    },
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::ResetTagFilter => {
                self.reset_tag_filter()?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::UpdateTagFilter, 
                        target: ActionTarget::Specific(vec![ComponentName::TagFilter])
                    },
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::FZFTagFilter => {
                self.set_tag_filter(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::UpdateTagFilter, 
                        target: ActionTarget::Specific(vec![ComponentName::TagFilter])
                    },
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    }
                ]);
            }

            Action::FZFStatusFilter => {
                self.set_custom_filter(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::CustomFilterUpdated, 
                        target: ActionTarget::Specific(vec![ComponentName::CurrentFilterInfo])
                    },
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::FZFOrderBy => {
                self.order_by(tui)?;
                return self.get_updated_table_actions();
            }

            Action::ShowCurrentlyTrackedTasks => {
                self.show_tracked_tasks(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::ToggleTimeTracking => {
                self.start_stop_task()?;
                return self.get_updated_table_actions();
            }

            Action::FZFFindTask => {
                self.find_task(tui)?;
                return self.get_updated_table_actions();
            }

            Action::ToggleDoneTasks => {
                self.toggle_hide_done()?;

                return Ok(vec![
                    self.get_hide_done_action()?,
                    self.get_show_tagless_action()?,
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::ToggleShowOnlyUntagged => {
                self.toggle_show_task_without_tags()?;
                return Ok(vec![
                    self.get_hide_done_action()?,
                    self.get_show_tagless_action()?,
                    RoutedAction {
                        action: self.get_task_updated_action()?,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::EditTask => {
                self.update_task(tui)?;
                return self.get_updated_table_actions();
            }

            Action::EditDueDate => {
                self.update_due_date(tui)?;
                return self.get_updated_table_actions();
            }

             Action::EditReminderDate => {
                self.update_reminder_date(tui)?;
                return self.get_updated_table_actions();
            }

             Action::EditStatus => {
                self.update_status(tui)?;
                return self.get_updated_table_actions();
            }

             Action::EditTimeTaken => {
                self.edit_time_taken(tui)?;
                return self.get_updated_table_actions();
            }
            _ => ()
        }
        Ok(vec![])
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, is_focused: bool) {
        let style = match is_focused {
            true => Style::default().fg(self.border_focused_color),
            false => Style::default().fg(self.border_default_color)
        };

        let block = Block::bordered()
            .style(Style::new()).title("tasks").border_style(style);
        self.table.block = Some(block);
        self.table.render_ref(area, frame.buffer_mut(), &mut self.state);
    }
}


