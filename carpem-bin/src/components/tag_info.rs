use std::{cell::RefCell, rc::Rc};
use anyhow::{anyhow, Context, Result};
use ratatui::{
    prelude::*, widgets::Block
};
use crate::{config::Config, utils::multi_list::{List, ListItem, ListState}};
use carpem_core::utils::{tag::Tag, task::Task};

use crate::{
    action::{Action, ActionTarget, RoutedAction}, app::Store, components::Component, tui::Tui, utils::user_interaction
};

use super::ComponentName;

pub struct CurrentTagsInfo<'a> {
    list: List<'a>,
    list_state: ListState,
    store: Option<Rc<RefCell<Store>>>,
    current_tags: Vec<Tag>,
    current_task: Option<Task>,
    current_task_index: Option<usize>,
    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
    tag_highlighted_fg: Color,
    tag_highlighted_bg: Color,
    tag_selected_fg: Color,
    tag_selected_bg: Color,
}

impl<'a> CurrentTagsInfo<'a> {
    pub fn new() -> Self {
        let list = List::default()
            .style(Color::White)
            .highlight_style(Modifier::REVERSED);

        Self {
            list,
            list_state: ListState::default(),
            store: None,
            current_tags: vec![],
            current_task: None,
            current_task_index: None,
            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
            tag_highlighted_fg: Color::Black,
            tag_highlighted_bg: Color::Yellow,
            tag_selected_fg: Color::Reset,
            tag_selected_bg: Color::White,
        }
    }

    pub fn add_tags(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(current_task) = &mut self.current_task else { return Ok(()); };
        let Some(task_id) = current_task.get_id() else { return Ok(()); };

        let mut tags_to_choose = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.get_tags().clone()
        };

        tags_to_choose.retain(|t| !self.current_tags.contains(t));
        let mut options = vec![];
        for (i, tag) in tags_to_choose.iter().enumerate() {
            let mut s = i.to_string();
            s.push_str(" ");
            s.push_str(&tag.text);
            options.push(s);
        }

        let selected_indices = user_interaction::fzf_select_indices(tui, &options)?;
        for &selection_index in selected_indices.iter() {
            let selected_tag = &tags_to_choose[selection_index];
            let Some(tag_id) = selected_tag.get_id() else { continue; };
            self.current_tags.push(selected_tag.clone());
            current_task.tags.push(tag_id);
        }
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.update_task(current_task.clone(), &self.current_task_index)?;
            self.current_tags = taskmanager.get_tags_for_task(task_id)?
        }
        self.update_view();
        return Ok(());
    }

    pub fn delete_tag(&mut self, tui: &mut Tui) -> Result<bool> {
        let Some(current_task) = &mut self.current_task else { return Ok(false); };

        let mut options = vec![];
        for (i, tag) in self.current_tags.iter().enumerate() {
            let mut s = i.to_string();
            s.push_str(" ");
            s.push_str(&tag.text);
            options.push(s);
        }

        let indices_to_remove = user_interaction::fzf_select_indices(tui, &options)?;
        self.current_tags = self.current_tags.clone()
            .into_iter()
            .enumerate()
            .filter(|(i,_)| !indices_to_remove.contains(i))
            .map(|(_, v)| v).collect();

        current_task.tags= self.current_tags.clone()
            .into_iter()
            .map(|tag| {
                let Some(tag_id) = tag.get_id() else {
                    return Err(anyhow!("no id found for tag: {:?}", tag)); 
                };
                Ok(tag_id)
            })
        .collect::<Result<Vec<_>, _>>()?; 

        let update_table = {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let amount_tasks_before = taskmanager.get_tasks().len();
            taskmanager.update_task(current_task.clone(), &self.current_task_index)?;
            amount_tasks_before != taskmanager.get_tasks().len()
        };

        self.update_view();
        Ok(update_table)
    }

    pub fn update_tags(&mut self) -> Result<()> {
        let Some(current_task) = &self.current_task else { return Ok(()); };
        let Some(task_id) = current_task.get_id() else { return Ok(()); };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            self.current_tags = taskmanager.get_tags_for_task(task_id)?;
        }
        self.update_view();
        Ok(())
    }

    pub fn update_current_task(&mut self, task: Option<Task>, index: Option<usize>) -> Result<()> {
        self.current_task = task;
        self.current_task_index = index;

        let Some(current_task) = &self.current_task else { 
            self.current_tags = vec![];
            self.update_view();
            return Ok(()); 
        };
        let Some(task_id) = current_task.get_id() else { return Ok(()); };
        {
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            self.current_tags = taskmanager.get_tags_for_task(task_id)?;
        }
        self.update_view();
        Ok(())
    }

    pub fn update_view(&mut self) {
        let mut list_items = vec![];
        for tag in self.current_tags.clone() {
            list_items.push(ListItem::new(tag.text));
        }

        self.list = List::new(list_items) 
            .style(Style::default().fg(self.text_color))
            .highlight_style(Style::default().fg(self.tag_highlighted_fg).bg(self.tag_highlighted_bg))
            .select_style(Style::default().fg(self.tag_selected_fg).bg(self.tag_selected_bg));
    }
}

impl<'a> Component for CurrentTagsInfo<'a> {
    fn name() -> ComponentName {
        ComponentName::CurrentTagsInfo
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
        self.tag_highlighted_fg = user_data.tag_highlighted_fg;
        self.tag_highlighted_bg= user_data.tag_highlighted_bg;
        self.tag_selected_fg = user_data.tag_selected_fg;
        self.tag_selected_bg = user_data.tag_selected_bg;
        Ok(())
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::NextTagInTask => {
                self.list_state.next();
                self.update_view();
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            } 
            
            Action::PrevTagInTask => {
                self.list_state.previous();
                self.update_view();
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            } 

            Action::CurrentTaskUpdated(task, index) => {
                self.update_current_task(task.clone(), *index)?;
            }

            Action::UpdateTagsInInfo => self.update_tags()?,
            Action::AddTagsToTask => {
                self.add_tags(tui)?; 
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    },
                ]);
            }

            Action::DeleteTagsFromTask => {
                let update_table = self.delete_tag(tui)?;
                if update_table {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::UpdateTaskTable,
                            target: ActionTarget::All
                        },
                        RoutedAction {
                            action:  Action::UpdateTagFilter,
                            target: ActionTarget::All
                        },
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        },
                    ]);
                } else {
                    return Ok(vec![
                        RoutedAction {
                            action: Action::Render,
                            target: ActionTarget::App
                        }
                    ]);
                }
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
            .style(Style::new()).title("tags for current task").border_style(style);

        self.list.block = Some(block);
        frame.render_stateful_widget(&self.list, area, &mut self.list_state);
    }
}
