use std::{cell::RefCell, rc::Rc};
use anyhow::{Context, Result};
use ratatui::{
    prelude::*, widgets::Block 
};
use carpem_core::utils::tag::Tag;

use crate::{
    action::{Action, ActionTarget, RoutedAction}, app::Store, components::Component, config::Config, tui::Tui, utils::multi_list
};

use crate::utils::user_interaction;
use super::ComponentName;

pub struct CurrentTagFilter<'a> {
    list: multi_list::List<'a>,
    list_state: multi_list::ListState,
    store: Option<Rc<RefCell<Store>>>,
    text_color: Color,
    border_focused_color: Color,
    border_default_color: Color,
    tag_highlighted_fg: Color,
    tag_highlighted_bg: Color,
    tag_selected_fg: Color,
    tag_selected_bg: Color,
}

impl<'a> CurrentTagFilter<'a> {
    pub fn new() -> Self {
        let list = multi_list::List::default()
            .style(Style::default().fg(Color::Reset))
            .highlight_style(Modifier::REVERSED);

        Self {
            list,
            list_state: multi_list::ListState::default(),
            store: None,
            text_color: Color::Reset,
            border_focused_color: Color::Yellow,
            border_default_color: Color::Gray,
            tag_highlighted_fg: Color::Black,
            tag_highlighted_bg: Color::Yellow,
            tag_selected_fg: Color::Reset,
            tag_selected_bg: Color::White,
        }
    }

    pub fn update_list(&mut self) -> Result<()> {
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
        let current_tags = taskmanager.get_tags();

        let mut list_items = vec![];
        for tag in current_tags.clone().iter() {
            list_items.push(multi_list::ListItem::new(tag.text.to_string()));
        }

        self.list = multi_list::List::new(list_items) 
            .style(Style::default().fg(self.text_color))
            .highlight_style(Style::default().fg(self.tag_highlighted_fg).bg(self.tag_highlighted_bg))
            .select_style(Style::default().fg(self.tag_selected_fg).bg(self.tag_selected_bg));

        let current_tag_filter = taskmanager.get_tag_filter().to_vec();

        // highlight all the right indices in list 
        self.list_state.selected_indices().clear();
        for (i, tag) in current_tags.iter().enumerate() {
            if let Some(id) = tag.get_id() {
                if current_tag_filter.contains(&id) {
                    self.list_state.select_index(i);
                }
            }
        }
        Ok(())
    }

    pub fn delete_tag(&mut self) -> Result<()> {
        let Some(current_list_index) = self.list_state.current_index() else { return Ok(()); };
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let Some(tag_id) = ({
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.get_tags()[current_list_index].get_id()
        }) else { return Ok(()); };
        {
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            taskmanager.delete_tag(tag_id)?;
        }
        self.update_list()?;

        Ok(())
    }

    pub fn add_tag(&mut self, tui: &mut Tui) -> Result<()> {
        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let tag_name = user_interaction::get_string_input(tui, "")?;

        // add it if it does not exist
        if !store.borrow().taskmanager.get_tags().iter().any(|t| t.text == tag_name) && !tag_name.trim().is_empty() { 
            let mut tag_to_add = Tag::new(None, &tag_name);
            {
                let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
                taskmanager.add_tag(&mut tag_to_add)?;
            }
            self.update_list()?;
        }

        Ok(())
    }

    pub fn update_tag(&mut self, tui: &mut Tui) -> Result<()> {
        {
            let Some(current_list_index) = self.list_state.current_index() else { return Ok(()); };
            let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
            let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
            let mut current_tag = taskmanager.get_tags()[current_list_index].clone();

            let new_tag_name = user_interaction::get_string_input(tui, &current_tag.text)?;

            // add it if it does not exist
            let update_current_tag = !taskmanager.get_tags().iter().any(|t| t.text == new_tag_name) && !new_tag_name.trim().is_empty();

            if update_current_tag {
                current_tag.text = new_tag_name;
                taskmanager.update_tag(current_tag.clone())?;
            }
        }
        self.update_list()?;

        Ok(())
    }

    pub fn toggle_current_tag(&mut self) -> Result<()> {
        let current_tag_is_selected = self.list_state.toggle();
        let Some(current_list_index) = self.list_state.current_index else { return Ok(()); };

        let store = self.store.as_ref().with_context(|| "could not access store to get taskmanager!")?;
        let taskmanager = &mut store.try_borrow_mut()?.taskmanager;
        let tags = taskmanager.get_tags();
        let id = match tags[current_list_index].get_id() {
            Some(id) => id,
            None => return Ok(())
        };

        let current_tags_filter = taskmanager.get_tag_filter_mut();

        // if the current tag is now selected then push the id into the filter
        // else remove the tag
        if current_tag_is_selected {
            current_tags_filter.push(id);
        } else {
            current_tags_filter.retain(|&tag_id| tag_id != id);
        }
        taskmanager.update()?;

        Ok(())
    }
}

impl<'a> Component for CurrentTagFilter<'a> {
    fn name() -> ComponentName {
        ComponentName::TagFilter
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

    fn init(&mut self, _area: Size) -> Result<Vec<RoutedAction>> {
        self.update_list()?;
        Ok(vec![])
    }

    fn update(&mut self, _prefix_number: usize, action: &Action, tui: &mut Tui) -> Result<Vec<RoutedAction>> { 
        match action {
            Action::UpdateTagFilter => {
                self.update_list()?;
            }

            Action::NextTag => {
                self.list_state.next();
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ])
            }

            Action::PrevTag => {
                self.list_state.previous();
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ])
            }

            Action::EditTag => {
                self.update_tag(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::UpdateTagsInInfo, 
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::UpdateTaskTable,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::DeleteTag => {
                self.delete_tag()?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::UpdateTagsInInfo, 
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::UpdateTaskTable,
                        target: ActionTarget::All
                    },
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::AddTag => {
                self.add_tag(tui)?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::Render,
                        target: ActionTarget::App
                    }
                ]);
            }

            Action::ToggleTag => {
                self.toggle_current_tag()?;
                return Ok(vec![
                    RoutedAction {
                        action: Action::UpdateTaskTable,
                        target: ActionTarget::All
                    },
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
            .style(Style::new()).title("tag filter").border_style(style);

        self.list.block = Some(block);
        frame.render_stateful_widget(&self.list, area, &mut self.list_state);
    }
}
