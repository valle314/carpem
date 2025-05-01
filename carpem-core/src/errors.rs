use chrono::NaiveDateTime;

#[derive(thiserror::Error, Debug)]
pub enum DataBaseError {
    #[error("could not update foreign_keys to value 1, current value = {0}")]
    ForeignKeyUpdateFailed(u8),

    #[error("task with title '{0:?}' has no ID")]
    TaskIdMissing(String),

    #[error("tag with title '{0:?}' has no ID")]
    TagIdMissing(String),

    #[error("event with title '{0:?}' has no ID")]
    EventIdMissing(String),
}

#[derive(thiserror::Error, Debug)]
pub enum TaskManagerError {
    #[error("task with title {0} does not have an id!")]
    MissingTaskId(String),

    #[error("tag with title {0} does not have an id!")]
    MissingTagId(String),

    #[error("cannot delete task: no task found with id {0}")]
    TaskIdNotFound(i64),
}

#[derive(thiserror::Error, Debug)]
pub enum EventManagerError {
    #[error("event with title {0} does not have an id!")]
    MissingEventId(String),

    #[error("invalid time: {0:02}:{1:02}:{2:02}")]
    InvalidTime(u32, u32, u32),

    #[error("could not add {0} day(s) to date {1}")]
    DateOverflow(u64, NaiveDateTime),
    
    #[error("could not create date {0}.{1}.{2}")]
    InvalidDate(i32, u32, u32),

    #[error("cannot delete event: no event found with id {0}")]
    EventIdNotFound(i64),
}
