use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Backlog,
    Pending,
    InProgress,
    Testing,
    Done,
}

impl Status {
    pub fn get_string(&self) -> String {
        match self {
            Status::Backlog => "Backlog".to_string(),
            Status::Pending => "Pending".to_string(),
            Status::InProgress => "InProgress".to_string(),
            Status::Testing => "Testing".to_string(),
            Status::Done => "Done".to_string(),
        }
    }
}
