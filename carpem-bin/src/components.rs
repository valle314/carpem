use std::{cell::RefCell, rc::Rc};
use anyhow::Result;
use ratatui::{
    layout::{Rect, Size},
    Frame,
};
use serde::{Deserialize, Serialize};

use crate::{
    action::{Action, RoutedAction}, app::Store, config::Config, tui::Tui
};

pub mod task;
pub mod tag_info;
pub mod tag_filter;
pub mod task_info;
pub mod custom_filter_info;
pub mod calendar;
pub mod calendar_info;
pub mod selected_date_time;
pub mod date_input;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Id {
    None,
    CompName(ComponentName),
    CompNameId { name: ComponentName, id: usize },
}

impl Id {
    pub fn get_name(&self) -> &ComponentName {
        match self {
            Self::CompName(name) => name,
            Self::CompNameId { name, id: _ } => name,
            Self::None => &ComponentName::NoName
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentName { // for keybindings
    Global,

    TaskViewGlobal, // for global keybindings in the task view
    TaskList,
    CurrentTaskInfo,
    CurrentFilterInfo,
    CurrentTagsInfo,
    TagFilter,

    EventViewGlobal, // for global keybindings in the event view
    Calendar,
    CurrentDayList,
    SelectedDate,

    // popups
    DateInput,

    NoName
}

pub trait Component {
    fn name() -> ComponentName
    where
        Self: Sized,
    {
        ComponentName::NoName
    }

    fn register_config_handler(&mut self, _config: Rc<RefCell<Config>>) -> Result<()> {
        Ok(())
    }

    fn register_store(&mut self, _store: Rc<RefCell<Store>>) -> Result<()>{
        Ok(())
    }

    fn init(&mut self, _area: Size) -> Result<Vec<RoutedAction>> {
        Ok(vec![])
    }

    fn handle_key_event(&mut self, _key: crossterm::event::KeyEvent, _handled: &mut bool, _tui: &mut Tui) -> Result<Vec<RoutedAction>> {
        Ok(vec![])
    }

    fn update(&mut self, _prefix_number: usize, _action: &Action, _tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        Ok(vec![])
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, is_focused: bool);
}
