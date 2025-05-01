use std::cmp::Ordering;

use crate::utils::status::Status;
use serde::{
    Serialize, Deserialize
};
use chrono::{
    NaiveDateTime, TimeDelta
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: Option<i64>,

    pub title: String,
    pub description: String,
    pub severity: i64,
    pub tags: Vec<i64>,
    pub status: Status,

    pub creation_date: NaiveDateTime,
    pub due_date: Option<NaiveDateTime>,
    pub reminder_date: Option<NaiveDateTime>,

    pub timer_start: Option<NaiveDateTime>,
    pub time_taken: Option<TimeDelta>
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TaskOrder {
    SeverityHighToLow,
    SeverityLowToHigh,
    ReminderDateHighToLow,
    ReminderDateLowToHigh,
    DueDateHighToLow,
    DueDateLowToHigh,
    TimeTakenHighToLow,
    TimeTakenLowToHigh,
}

impl TaskOrder {
    pub fn compare_tasks(&self, a: &Task, b: &Task) -> Ordering {
        match self {
            TaskOrder::SeverityHighToLow => b.severity.cmp(&a.severity),
            TaskOrder::SeverityLowToHigh => a.severity.cmp(&b.severity),

            TaskOrder::ReminderDateHighToLow => b.reminder_date.cmp(&a.reminder_date),
            TaskOrder::ReminderDateLowToHigh => a.reminder_date.cmp(&b.reminder_date),

            TaskOrder::DueDateHighToLow => b.due_date.cmp(&a.due_date),
            TaskOrder::DueDateLowToHigh => a.due_date.cmp(&b.due_date),

            TaskOrder::TimeTakenHighToLow => b.time_taken.cmp(&a.time_taken),
            TaskOrder::TimeTakenLowToHigh => a.time_taken.cmp(&b.time_taken),
        }
    }
}

impl Task {
    pub fn new(id: Option<i64>,
        title: &str,
        description: &str,
        severity: i64,
        tags: Vec<i64>,
        status: Status,
        creation_date: NaiveDateTime,
        due_date: Option<NaiveDateTime>,
        reminder_date: Option<NaiveDateTime>,
        timer_start: Option<NaiveDateTime>,
        time_taken: Option<TimeDelta>) -> Self {
        Self {
            id, 
            title: title.to_string(),
            description: description.to_string(),
            severity,
            tags,
            status,
            creation_date,
            due_date,
            reminder_date,
            timer_start,
            time_taken
        }
    }

    pub fn get_id(&self) -> Option<i64> {
        self.id
    }

    pub fn set_id(&mut self, id: i64) {
        if self.id.is_none() {
            self.id = Some(id);
        }
        else {
            panic!("Cannot set id for Task: {:?} that already has one, current id: {:?}", self.title, self.id);
        }
    }
}
