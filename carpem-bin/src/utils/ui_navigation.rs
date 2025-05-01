use std::{cell::RefCell, collections::HashMap, rc::Rc};
use crate::components::{calendar::Calendar, calendar_info::CalendarInfo, custom_filter_info::CustomFilterInfo, date_input::DateInput, selected_date_time::SelectedDate, tag_filter::CurrentTagFilter, tag_info::CurrentTagsInfo, task::TaskComponent, task_info::CurrentTaskInfo, Component, ComponentName, Id};
use anyhow::{Context, Result};
use ratatui::{layout::{self, Constraint, Layout, Rect}, Frame};

#[derive(PartialEq, Copy, Eq, Clone, Hash, Debug)]
pub enum View {
    TaskView,
    CalendarView,
    PopupView,
}

impl View {
    pub fn get_name(&self) -> ComponentName {
        match self {
            Self::TaskView => ComponentName::TaskViewGlobal,
            Self::CalendarView => ComponentName::EventViewGlobal,
            Self::PopupView => ComponentName::DateInput
        }
    }

    pub fn get_default_active_id(&self) -> Id {
        match self {
            Self::TaskView => Id::CompName(TaskComponent::name()),
            Self::CalendarView => Id::CompName(Calendar::name()),
            Self::PopupView => Id::CompName(DateInput::name()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct UINavigation {
    ui_grid_map: HashMap<View, HashMap<Id, HashMap<Direction, Id>>>,
    all_views: Vec<View>,
    pub components: HashMap<View, HashMap<Id, Rc<RefCell<Box<dyn Component>>>>>, 

    current_view_index: usize,
    pub current_view: View,
    pub current_component_id: Id,

    pub popup: Option<Id>
}

impl UINavigation {
    pub fn new() -> Result<Self> {
        let components: HashMap<View, HashMap<Id, Rc<RefCell<Box<dyn Component>>>>> = HashMap::from([
            (
                View::TaskView, HashMap::from([
                    Self::build_component(TaskComponent::new()),
                    Self::build_component(CurrentTagsInfo::new()),
                    Self::build_component(CurrentTagFilter::new()),
                    Self::build_component(CurrentTaskInfo::new()),
                    Self::build_component(CustomFilterInfo::new()),
                ])
            ),
            (
                View::CalendarView, HashMap::from([
                Self::build_component(Calendar::new()),
                Self::build_component(CalendarInfo::new()),
                Self::build_component(SelectedDate::new()),
                ])
            ),
            (
                View::PopupView, HashMap::from([
                Self::build_component(DateInput::new()),
                ])
            )
        ]);

        // let all_views = vec![View::TaskView, View::CalendarView, View::HelpView];
        let all_views = vec![View::TaskView, View::CalendarView];
        let current_view_index = 0;
        let current_view = all_views[current_view_index];

        let current_component_id = current_view.get_default_active_id();

        let ui_grid_map: HashMap<View, HashMap<Id, HashMap<Direction, Id>>> = HashMap::new();
        let mut navigation = Self { 
            ui_grid_map,
            all_views,
            current_view_index,
            current_component_id,
            current_view,
            components,
            popup: None
        };

        // task view 
        let mut component_map = HashMap::new();
        component_map.insert("task-list", Id::CompName(ComponentName::TaskList));
        component_map.insert("task-info", Id::CompName(ComponentName::CurrentTaskInfo));
        component_map.insert("current-tags", Id::CompName(ComponentName::CurrentTagsInfo));
        component_map.insert("tag-filter", Id::CompName(ComponentName::TagFilter));
        component_map.insert("current-filter", Id::CompName(ComponentName::CurrentFilterInfo));

        // this layout defines only the components that can be focused (i.e., "active").
        // some components are intentionally left out to prevent them from being focusable
        // so, this is not the full ui layout — just the subset for focus navigation.
        let grid = Self::transform_to_ids(vec![
            vec!["task-list",       "current-tags"],
            vec!["task-list",      "tag-filter"],
        ], component_map)?;
        navigation.add_navigation_view(View::TaskView, grid);

        // calendar view
        let mut component_map = HashMap::new();
        component_map.insert("calendar", Id::CompName(ComponentName::Calendar));
        component_map.insert("current-day", Id::CompName(ComponentName::CurrentDayList));
        component_map.insert("selected-date", Id::CompName(ComponentName::SelectedDate));

        let grid = Self::transform_to_ids(vec![
            vec!["calendar"],
            vec!["current-day"],
        ], component_map)?;
        navigation.add_navigation_view(View::CalendarView, grid);

        // help view
        // let mut component_map = HashMap::new();
        // component_map.insert("help", Id::CompName(ComponentName::Help));
        //
        // let grid = Self::transform_to_ids(vec![
        //     vec!["help"],
        // ], component_map)?;
        // navigation.add_navigation_view(View::PopupView, grid);

        Ok(navigation)
    }

    pub fn current_component(&self) -> Result<Rc<RefCell<Box<dyn Component>>>> {
        Ok( self.components.get(&self.current_view)
            .with_context(|| format!("no components found at current view: {:?}", self.current_view))?
            .get(&self.current_component_id)
            .with_context(|| format!("no component found with id: {:?}", self.current_component_id))?.clone() )
    }

    pub fn set_popup(&mut self, component_name: ComponentName) {
        self.popup = Some(Id::CompName(component_name));
    }

    pub fn remove_popup(&mut self) {
        self.popup = None;
    }

    pub fn get_popup_component(&self) -> Option<Rc<RefCell<Box<dyn Component>>>> {
        let Some(popup) = self.popup else { return None; };
        self.components.get(&View::PopupView).and_then(|view_map| view_map.get(&popup).cloned())
    }

    fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
        let [area] = Layout::horizontal([horizontal])
            .flex(layout::Flex::Center)
            .areas(area);
        let [area] = Layout::vertical([vertical]).flex(layout::Flex::Center).areas(area);
        area
    }

    pub fn id_to_rect(frame: &mut Frame, component_id: &Id) -> Rect {
        // task layout
        let outer_layout = Self::layout_h(frame.area(), vec![Constraint::Fill(2), Constraint::Fill(1)]);
        let left_layout = Self::layout_v(outer_layout[0], vec![Constraint::Fill(1), Constraint::Length(9)]);
        let right_layout = Self::layout_v(outer_layout[1], vec![Constraint::Fill(1), Constraint::Length(9), Constraint::Length(9)]);
        // let filter_layout = Self::layout_h(left_layout[1], vec![Constraint::Fill(1), Constraint::Fill(1)]);

        // calendar layout
        let calendar_outer_layout = Self::layout_v(frame.area(), vec![layout::Constraint::Length(10), layout::Constraint::Fill(1)]);
        let calendar_top_layout = Self::layout_h(calendar_outer_layout[0], vec![
            layout::Constraint::Fill(1),
            layout::Constraint::Length(31),
        ]);

        let input_area = Self::center(
            frame.area(),
            Constraint::Percentage(40),
            Constraint::Length(3), 
        );

        match component_id {
            Id::CompName(name) => {
                match name {
                    // task view
                    ComponentName::TaskList => left_layout[0],
                    ComponentName::CurrentFilterInfo => left_layout[1],
                    ComponentName::TagFilter => right_layout[2],
                    ComponentName::CurrentTaskInfo => right_layout[0],
                    ComponentName::CurrentTagsInfo => right_layout[1],

                    // calendar view
                    ComponentName::SelectedDate => calendar_top_layout[1],
                    ComponentName::Calendar => calendar_top_layout[0],
                    ComponentName::CurrentDayList => calendar_outer_layout[1],

                    // popup
                    // ComponentName::Help => help_area,
                    ComponentName::DateInput => input_area,
                    _ => layout::Rect::new(0, 0, 0, 0)
                }
            }

            Id::CompNameId{name, id} => {
                match (name, id) {
                    // calendar view
                    // (ComponentName::SelectedDate, 0) => calendar_top_layout[0],
                    // (ComponentName::SelectedDate, 1) => calendar_top_layout[2],

                    _ => layout::Rect::new(0, 0, 0, 0)
                }
            }
            _ => layout::Rect::new(0, 0, 0, 0)
        }
    }

    fn layout_h(area: Rect, constraints: Vec<layout::Constraint>) -> Rc<[Rect]> {
        layout::Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(constraints)
            .split(area)
    }

    fn layout_v(area: Rect, constraints: Vec<layout::Constraint>) -> Rc<[Rect]> {
        layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(constraints)
            .split(area)
    }

    fn build_component<T: Component + 'static>(c: T) -> (Id, Rc<RefCell<Box<dyn Component>>>) {
        let name = T::name();
        (Id::CompName(name), Rc::new(RefCell::new(Box::new(c))))
    }

    fn _build_component_with_id<T: Component + 'static>(c: T, id: usize) -> (Id, Rc<RefCell<Box<dyn Component>>>) {
        let name = T::name();
        (Id::CompNameId { name, id} ,Rc::new(RefCell::new(Box::new(c))))
    }

    pub fn next_view(&mut self) -> Result<()> {
        self.current_view_index += 1;
        self.current_view_index = self.current_view_index % self.all_views.len();
        self.current_view = self.all_views[self.current_view_index];
        self.current_component_id = self.current_view.get_default_active_id();
        Ok(())
    }

    pub fn move_to(&mut self, direction: &Direction) -> Result<()> {
        let id = self.ui_grid_map.get(&self.current_view)
            .and_then(|view_map| view_map.get(&self.current_component_id))
            .and_then(|component_map| component_map.get(direction))
            .with_context(|| format!(
                    "unable to navigate: could not find valid target component for direction {:?} from current component {:?} in view {:?}",
                    direction,
                    self.current_component_id,
                    self.current_view
            ))?;

        if *id == Id::None {
            return Ok(());
        }

        self.current_component_id = id.clone();
        Ok(())
    }

    fn transform_to_ids(grid: Vec<Vec<&str>>, component_map: HashMap<&str, Id>) -> Result<Vec<Vec<Id>>> {
        let mut res = vec![];
        for rows in grid.iter() {
            let mut row_vec = vec![];
            for key in rows.iter() {
                if key.is_empty() {
                    row_vec.push(Id::None);
                    continue;
                }
                let id = *component_map.get(key)
                    .with_context(|| format!("failed to get value from key: {key:?}"))?;
                row_vec.push(id);
            }
            res.push(row_vec);
        }

        Ok(res)
    }

    fn add_navigation_view(
        &mut self,
        view: View,
        grid: Vec<Vec<Id>>,
    ) {
        let mut view_map: HashMap<Id, HashMap<Direction, Id>> = HashMap::new();

        for (row_idx, row) in grid.iter().enumerate() {
            for (col_idx, &comp_id) in row.iter().enumerate() {
                let mut direction_map = HashMap::new();

                // up
                if row_idx > 0 {
                    direction_map.insert(Direction::Up, grid[row_idx - 1][col_idx]);
                } else {
                    direction_map.insert(Direction::Up, comp_id);
                }

                // down
                if row_idx < grid.len() - 1 {
                    direction_map.insert(Direction::Down, grid[row_idx + 1][col_idx]);
                } else {
                    direction_map.insert(Direction::Down, comp_id);
                }

                // left
                if col_idx > 0 {
                    direction_map.insert(Direction::Left, grid[row_idx][col_idx - 1]);
                } else {
                    direction_map.insert(Direction::Left, comp_id);
                }

                // right
                if col_idx < row.len() - 1 {
                    direction_map.insert(Direction::Right, grid[row_idx][col_idx + 1]);
                } else {
                    direction_map.insert(Direction::Right, comp_id);
                }

                view_map.insert(comp_id, direction_map);
            }
        }

        self.ui_grid_map.insert(view, view_map);
    }
}
