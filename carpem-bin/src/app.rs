use std::rc::Rc;
use std::cell::RefCell;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets;
use ratatui::prelude::Rect;
use carpem_core::{utils::database::Database, EventManager, TaskManager};
use tokio::sync::mpsc;
use tracing::info;

use crate::action::{ActionTarget, RoutedAction};
use crate::components::ComponentName;
use crate::utils::ui_navigation::{self, UINavigation};
use crate::{
    action::Action,
    config,
    config::{Config, KeyBindings},
    tui::{Event, Tui},
};

pub struct Store {
    pub taskmanager: TaskManager,
    pub eventmanager: EventManager
}

pub struct App {
    config: Rc<RefCell<Config>>,
    tick_rate: f64,
    frame_rate: f64,
    store: Rc<RefCell<Store>>,
    current_prefix: usize,
    should_reset_prefix: bool,
    key_presses_since_should_reset: usize,
    should_quit: bool,
    ui_navigation: UINavigation,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<RoutedAction>,
    action_rx: mpsc::UnboundedReceiver<RoutedAction>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let cfg = Config::new()?;
        let cfg = Rc::new(RefCell::new(cfg));

        let data_dir = config::get_data_dir();
        let database_path = data_dir.join("database.db");
        let database_path_str = database_path.to_string_lossy();
        if !database_path.exists() {
            match std::fs::File::create(&database_path) {
                Ok(_) => info!("created database file: {}", database_path_str),
                Err(e) => info!("failed to create database at {} and got error:\n{}", 
                    database_path_str, e),
            }
        } else {
            info!("found database: {}", database_path_str);
        }

        let db = Database::new(Some(&database_path_str))?; 

        // let mut db = Database::new(None)?;   // this is for 
        // db.fill_database()?;                 // debug build

        let db_ref = Rc::new(RefCell::new(db));
        let mut taskmanager = TaskManager::new(Rc::clone(&db_ref));
        taskmanager.update()?;

        let mut eventmanager = EventManager::new(Rc::clone(&db_ref));
        eventmanager.update()?;

        let store = Rc::new(RefCell::new( Store {
            taskmanager,
            eventmanager
        }));

        let ui_navigation = UINavigation::new()?;

        Ok(Self {
            tick_rate,
            frame_rate,
            store,
            should_quit: false,
            config: cfg,
            current_prefix: 0,
            should_reset_prefix: false,
            key_presses_since_should_reset: 0,
            ui_navigation,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
        })
    }

    fn send_actions(&self, actions_to_send: Vec<RoutedAction>) -> Result<()>{
        for routed_action in actions_to_send.into_iter() {
            self.action_tx.send(routed_action).with_context(|| "failed to send action!")?;
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut tui = Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        info!("{:?}", self.config);

        for (_, component) in self.ui_navigation.components.values_mut().flatten() {
            component.try_borrow_mut()?.register_config_handler(self.config.clone())?;
        }

        for (_, component) in self.ui_navigation.components.values_mut().flatten() {
            component.try_borrow_mut()?.register_store(Rc::clone(&self.store))?;
        }

        let mut init_actions = vec![];
        for (_, component) in self.ui_navigation.components.values_mut().flatten() {
            let actions_to_send = component.try_borrow_mut()?.init(tui.size()?)?;
            for action in actions_to_send.into_iter() {
                init_actions.push(action);
            }
        }
        self.send_actions(init_actions)?;

        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
            if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(RoutedAction { action: Action::Quit, target: ActionTarget::App })?,
            Event::Tick => action_tx.send(RoutedAction { action: Action::Tick, target: ActionTarget::All })?,
            Event::Render => action_tx.send(RoutedAction { action: Action::Render, target: ActionTarget::App })?,
            Event::Resize(x, y) => action_tx.send(RoutedAction { action: Action::Resize(x, y), target: ActionTarget::App })?,
            Event::Key(key) => self.handle_key_event(&key, tui)?,
            _ => {}
        }

        Ok(())
    }

    fn handle_keymap_action_event(&mut self, component_name: ComponentName, target: ActionTarget) -> Result<bool> { 
        let keybindings = self.config.try_borrow_mut()?.clone().keybindings;
        let Some(keymap) = keybindings.get(&component_name) else { return Ok(false); };
        let current_len = self.last_tick_key_events.len();

        for i in 0..current_len {
            let keysequence_to_check = &self.last_tick_key_events[i..current_len];
            if let Some(action) = keymap.get(keysequence_to_check) {
                self.last_tick_key_events.drain(..);
                self.send_actions( vec![
                        RoutedAction {
                            action: action.clone(),
                            target
                        }
                ])?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn reset_prefix(&mut self) {
        self.key_presses_since_should_reset = 0;
        self.should_reset_prefix = false;
        self.current_prefix = 0; 
    }

    fn handle_popup_keyevents(&mut self, tui: &mut Tui, key: &KeyEvent) -> Result<()>{
        let mut handled = false; 
        let Some(popup_compoment) = self.ui_navigation.get_popup_component() else { return Ok(()); };
        let routed_actions = popup_compoment.try_borrow_mut()?.handle_key_event(*key, &mut handled, tui)?; 
        if handled {
            self.last_tick_key_events.drain(..);
            self.send_actions(routed_actions)?;
            return Ok(());
        }

        if self.handle_keymap_action_event(ComponentName::Global, ActionTarget::App)? { 
            return Ok(());
        } 

        Ok(())
    }

    fn handle_key_event(&mut self, key: &KeyEvent, tui: &mut Tui) -> Result<()> {
        match (key.code, self.should_reset_prefix) {
            (KeyCode::Char(c), false) => {
                if let Some(digit) = c.to_digit(10) {
                    self.current_prefix = self.current_prefix.saturating_mul(10);
                    self.current_prefix = self.current_prefix.saturating_add(digit as usize);
                } else {
                    self.should_reset_prefix = true;
                }
            }
            (KeyCode::Esc, _) => self.reset_prefix(),
            _ => ()
        }
        self.last_tick_key_events.push(*key);
        if self.should_reset_prefix {
            self.key_presses_since_should_reset += 1;
            if self.key_presses_since_should_reset >= KeyBindings::MAX_SEQUENCE_LENGTH {
                self.reset_prefix();
            }
        }

        if self.ui_navigation.popup.is_some() {
            self.handle_popup_keyevents(tui, key)?;
            return Ok(())
        }

        // check if the active component generates an action from user defined key combinations
        let target = ActionTarget::View(self.ui_navigation.current_view.clone()); 
        if self.handle_keymap_action_event(*self.ui_navigation.current_component_id.get_name(), target)? { 
            return Ok(());
        } 

        let mut handled = false; 
        let routed_actions = self.ui_navigation.current_component()?.try_borrow_mut()?.handle_key_event(*key, &mut handled, tui)?; 
        if handled {
            self.last_tick_key_events.drain(..);
            self.send_actions(routed_actions)?;
            return Ok(());
        }


        // self.current_view_name
        let target = ActionTarget::View(self.ui_navigation.current_view.clone()); 
        if self.handle_keymap_action_event(self.ui_navigation.current_view.get_name(), target)? { 
            return Ok(()); 
        } 

        // global keymap events (switching views for example)
        if self.handle_keymap_action_event(ComponentName::Global, ActionTarget::App)? { 
            return Ok(());
        } 

        return Ok(()); 
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.action_tx.send(RoutedAction { action: Action::Render, target: ActionTarget::App })?; 
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> Result<()> {
        tui.draw(|frame| {
            if let Some(components) = self.ui_navigation.components.get_mut(&self.ui_navigation.current_view) {
                for (id, component) in components.iter_mut() {
                    let is_focused = self.ui_navigation.current_component_id == *id;
                    let rect = UINavigation::id_to_rect(frame, id);
                    component.borrow_mut().draw(frame, rect, is_focused);
                }
            }

            match self.ui_navigation.popup {
                Some(popup) => {
                    let rect = UINavigation::id_to_rect(frame, &popup);
                    frame.render_widget(widgets::Clear, rect);
                    match self.ui_navigation.get_popup_component() {
                        Some(c) => c.borrow_mut().draw(frame, rect, true),
                        None => ()
                    }
                }
                None => ()
            }
        })?;
        Ok(())
    }

    // ------------------ application dependent ------------------
    fn handle_global_app_action(&mut self, tui: &mut Tui, action: &Action) -> Result<()> { 
        let mut focus_changed = false;
        match action {
            Action::ClearScreen =>{tui.terminal.clear()?;}
            Action::Resize(w, h) => self.handle_resize(tui, *w, *h)?,
            Action::Render => self.render(tui)?,
            Action::QuitHelp => self.action_tx.send(RoutedAction { action: Action::ViewMain, target: ActionTarget::All })?,
            Action::Quit => self.should_quit = true,

            Action::FocusLeft => {
                self.ui_navigation.move_to(&ui_navigation::Direction::Left)?;
                focus_changed = true;
            }

            Action::FocusRight => {
                self.ui_navigation.move_to(&ui_navigation::Direction::Right)?;
                focus_changed = true;
            }

            Action::FocusUp => {
                self.ui_navigation.move_to(&ui_navigation::Direction::Up)?;
                focus_changed = true;
            }

            Action::FocusDown => {
                self.ui_navigation.move_to(&ui_navigation::Direction::Down)?;
                focus_changed = true;
            }

            Action::NextView => {
                self.ui_navigation.next_view()?;
                focus_changed = true;
            }

            Action::Tick => {
                while self.last_tick_key_events.len() > KeyBindings::MAX_SEQUENCE_LENGTH {
                    self.last_tick_key_events.remove(0);
                }
            }

            Action::Popup(component_name) => self.ui_navigation.set_popup(*component_name), 
            Action::QuitPopup => self.ui_navigation.remove_popup(),
            _ => ()
        }

        if focus_changed {
            self.action_tx.send(RoutedAction { action: Action::Render, target: ActionTarget::App })?;
        }

        Ok(())
    }

    fn handle_actions(&mut self, tui: &mut Tui) -> Result<()> {
        let mut reset_prefix = false;
        while let Ok(routed_action) = self.action_rx.try_recv() {
            let mut actions_to_send = vec![];
            let action = &routed_action.action;
            match &routed_action.target {
                ActionTarget::App => self.handle_global_app_action(tui, action)?,
                ActionTarget::All => { 
                    for (_, component) in self.ui_navigation.components.values_mut().flatten() {
                        let mut actions = component.try_borrow_mut()?.update(self.current_prefix, &action, tui)?;
                        actions_to_send.append(&mut actions);
                    }
                }

                ActionTarget::View(view) => {
                    let Some(components) = self.ui_navigation.components.get_mut(&view) else { continue; };
                    for (_, component) in components.iter_mut() {
                        let mut actions = component.try_borrow_mut()?.update(self.current_prefix, &action, tui)?;
                        actions_to_send.append(&mut actions);
                    } 
                    reset_prefix = true;
                }

                ActionTarget::Specific(component_names) => {
                    for (id, component) in self.ui_navigation.components.values_mut().flatten() {
                        if component_names.contains(id.get_name()) {
                            let mut actions = component.try_borrow_mut()?.update(self.current_prefix, &action, tui)?;
                            actions_to_send.append(&mut actions);
                        }
                    }
                    reset_prefix = true;
                }

                ActionTarget::Active => {
                    let mut actions = self.ui_navigation.current_component()?.try_borrow_mut()?.update(self.current_prefix, &action, tui)?;
                    actions_to_send.append(&mut actions);
                    reset_prefix = true;
                }
            }
            if !actions_to_send.is_empty() { self.send_actions(actions_to_send)?; }
        }

        if reset_prefix {
            self.reset_prefix();
        }

        Ok(())
    }
}
