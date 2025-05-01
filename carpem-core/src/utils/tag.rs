use serde::{
    Serialize, Deserialize
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tag {
    id: Option<i64>,
    pub text: String
}

impl Tag {
    pub fn new(id: Option<i64>, text: &str) -> Self {
        Self { id, text: text.to_string() }
    }

    pub fn get_id(&self) -> Option<i64> {
        self.id
    }

    pub fn set_id(&mut self, id: i64) {
        if self.id.is_none() {
            self.id = Some(id);
        }
        else {
            panic!("Cannot set id for tag: {:?} that already has one, current id: {:?}", self.text, self.id);
        }
    }

}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Tag { }
