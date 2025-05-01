use chrono::NaiveDateTime;
use carpem_core::utils::task::Task;
use serde::{Deserialize, Serialize};

use crate::{components::ComponentName, utils::ui_navigation::View};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Quit,
    QuitHelp,
    ClearScreen,
    Error(String),

    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,

    Popup(ComponentName),
    QuitPopup,

    // event related
    GoToToday,
    GoToPrevDay,
    GoToNextDay,
    GoToPrevWeek,
    GoToNextWeek,
    FindDate,
    FZFFindEvent,
    AddEvent,
    NextEvent,
    PrevEvent,
    EditEvent,
    EditDurationTime,
    EditRecurrence,
    EditDate,
    DeleteEvent,


    // task related
    AddTask,
    DeleteTask,
    EditTask,
    DeleteAllDoneTasks,
    ToggleTimeTracking,
    FZFFindTask,
    ToggleDoneTasks,
    ResetDueDate,
    ResetReminderDate,
    ResetTimeTaken,
    EditReminderDate,
    EditStatus,
    EditTimeTaken,
    EditDueDate,
    NextTask,
    PrevTask,
    SelectFirst,
    SelectLast,


    // filter related
    ResetTagFilter,
    ResetStatusFilter,
    FZFTagFilter,
    FZFStatusFilter,
    FZFOrderBy,
    ShowCurrentlyTrackedTasks,
    ToggleShowOnlyUntagged,

    // tag related
    AddTagsToTask,
    DeleteTagsFromTask,
    PrevTagInTask,
    NextTagInTask,
    NextTag,
    PrevTag,
    EditTag,
    DeleteTag,
    AddTag,
    ToggleTag,

    // for the application only
    UpdateTaskTable,
    CurrentTaskUpdated(Option<Task>, Option<usize>),
    CurrentDateUpdated(NaiveDateTime),
    UpdateTagFilter,
    UpdateTagsInInfo,
    CustomFilterUpdated,
    SelectEventIndex(usize),
    UpdateCurrentDay,

    HideAllStatus(bool),
    ShowOnlyTaglessStatus(bool),

    ViewMain,
    ViewHelp,
    ViewCalendar,

    NextView,
    PrevView
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum ActionTarget {
    All,
    Specific(Vec<ComponentName>),
    Active,
    App, 
    View(View)
}

#[derive(Debug, Clone)]
pub struct RoutedAction {
    pub action: Action,
    pub target: ActionTarget,
}
