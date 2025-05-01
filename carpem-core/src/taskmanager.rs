use std::{
    cell::RefCell,rc::Rc
};
use anyhow::Result;

use crate::{errors::TaskManagerError, utils::{
    database::Database, status::Status, tag::Tag, task::{Task, TaskOrder}
}};

pub struct TaskManager {
    db: Rc<RefCell<Database>>,
    tasks: Vec<Task>,
    hide_done: bool,
    show_only_tagless: bool,
    current_ordering: TaskOrder,
    tags: Vec<Tag>,
    tags_filter: Vec<i64>,
    sql_filter: Option<String>
}

impl TaskManager {
    pub fn new(db: Rc<RefCell<Database>>) -> Self {
        Self {
            db,
            tasks: vec![],
            hide_done: false,
            show_only_tagless: false,
            current_ordering: TaskOrder::SeverityHighToLow,
            tags: vec![],
            tags_filter: vec![],
            sql_filter: None
        }
    }

    pub fn get_tasks(&self) -> &Vec<Task> {
        &self.tasks
    }

    pub fn get_tracked_tasks(&self) -> Result<Vec<Task>> {
        let tracked_tasks_ids = self.db.borrow_mut().get_tracked_tasks()?;
        let mut task_vec = vec![];
        for &id in tracked_tasks_ids.iter() {
            task_vec.push(self.get_task_from_id(id)?);
        }
        Ok(task_vec)
    }

    pub fn get_tags(&self) -> &Vec<Tag> {
        &self.tags
    }

    pub fn get_id_from_tag_text(&self, text: &str) -> Result<i64> {
        self.db.borrow_mut().get_id_from_tag_text(text)
    }

    pub fn toggle_hide_done(&mut self) -> Result<()> {
        self.hide_done = !self.hide_done;
        if self.hide_done {
            self.show_only_tagless = false;
        }
        self.update()?;
        Ok(())
    }

    pub fn get_hide_done_status(&self) -> bool {
        self.hide_done
    }

    pub fn toggle_show_task_without_tags(&mut self) -> Result<bool> {
        self.show_only_tagless = !self.show_only_tagless;
        if self.show_only_tagless {
            self.hide_done = false;
        }
        self.update()?;
        Ok(self.show_only_tagless)
    }

    pub fn get_show_tagless_status(&self) -> bool {
        self.show_only_tagless
    }

    pub fn update(&mut self) -> Result<()> {
        let sql_filter = match &self.sql_filter {
            None => None,
            Some(sql_filter) => Some(sql_filter.as_str())
        };

        self.tasks = self.db.borrow_mut().get_filtered_tasks(&self.tags_filter, sql_filter)?;
        match (self.hide_done, self.show_only_tagless) {
            (_, true) => self.tasks.retain(|t| t.tags.is_empty()), // show all tasks without tags
            (true, false) => self.tasks.retain(|t| t.status != Status::Done), // hide tasks that are done 
            _ => ()
        }

        self.tags = self.db.borrow_mut().get_tags()?;
        self.tags.sort_by(|a, b| a.text.cmp(&b.text));
        self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
        Ok(())
    }

    pub fn get_task_from_id(&self, task_id: i64) -> Result<Task> {
        self.db.borrow_mut().get_task_from_id(task_id)
    }

    pub fn set_ordering(&mut self, ordering: TaskOrder) {
        self.current_ordering = ordering;
        self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
    }

    pub fn set_filter(&mut self, filtered_tags: Vec<i64>, custom_filter: Option<&str>) -> Result<()> { 
        self.tags_filter = filtered_tags;
        self.sql_filter = match custom_filter {
            None => None,
            Some(filter) => Some(filter.to_string())
        };
        self.update()?;
        Ok(())
    }

    pub fn set_custom_filter(&mut self, custom_filter: Option<&str>) -> Result<()> { 
        self.sql_filter = match custom_filter {
            None => None,
            Some(filter) => Some(filter.to_string())
        };
        self.update()?;
        Ok(())
    }

    pub fn set_tags_filter(&mut self, filtered_tags: Vec<i64>) -> Result<()> {
        self.tags_filter = filtered_tags;
        self.update()?;
        Ok(())
    }


    pub fn get_custom_filter(&self) -> Option<String> {
        self.sql_filter.clone()
    }
    
    pub fn get_tag_filter(&self) -> &Vec<i64> {
        &self.tags_filter
    }

    pub fn get_tag_filter_mut(&mut self) -> &mut Vec<i64> {
        &mut self.tags_filter
    }

    pub fn get_tags_for_task(&self, taks_id: i64) -> Result<Vec<Tag>> { 
        self.db.borrow_mut().get_tags_for_task(taks_id)
    }

    pub fn get_current_task(&self, task_index: &Option<usize>) -> Option<&Task> {
        if let Some(index) = task_index {
            self.tasks.get(*index)
        } else { None }
    }

    pub fn delete_task(&mut self, task_index: usize) -> Result<()> { 
        if let Some(task) = self.tasks.get(task_index) {
            if let Some(task_id) = task.get_id() {
                self.db.borrow_mut().delete_task(task_id)?;
                self.tasks.remove(task_index);
            }
        }
        Ok(())
    }

    pub fn delete_done_tasks(&mut self) -> Result<()> {
        self.db.borrow_mut().delete_done_tasks()?;
        self.update()?;
        Ok(())
    }

    pub fn delete_task_with_id(&mut self, task: Task) -> Result<()> {
        let Some(task_id) = task.get_id() else {
            return Err(TaskManagerError::MissingTaskId(task.title.clone()).into());
        };

        let index = self.tasks.iter().position(|task| {
            let Some(id) = task.get_id() else { return false; };
            task_id == id
        }).ok_or_else(|| TaskManagerError::TaskIdNotFound(task_id))?;

        self.delete_task(index)?;
        Ok(())
    }

    pub fn add_task(&mut self, mut task: Task) -> Result<Option<i64>> {
        self.db.borrow_mut().add_task(&mut task)?;

        let Some(task_id) = task.get_id() else {
            return Err(TaskManagerError::MissingTaskId(task.title.clone()).into());
        };

        if self.show_only_tagless {
            if task.tags.is_empty() {
                self.tasks.push(task);
                self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
            }
            return Ok(None);
        }

        let passes_tags_filter = self.db.borrow_mut().task_passes_tag_filter(task_id, &self.tags_filter)?;
        let passes_sql_filter = match self.sql_filter {
            None => true,
            Some(ref filter) => self.db.borrow_mut().task_passes_custom_filter(task_id, filter)?
        };
        let passes_hide_done = self.task_passes_hide_done_filter(&task);

        if passes_tags_filter && passes_sql_filter && passes_hide_done {
            self.tasks.push(task);
            self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
        }

        Ok(Some(task_id))
    }

    fn task_passes_hide_done_filter(&self, task: &Task) -> bool { 
        !(self.hide_done && task.status == Status::Done)
    }

    pub fn update_task(&mut self, mut task: Task, task_index: &Option<usize>) -> Result<()> {
        if let Some(index) = task_index {
            let current_task = &self.tasks[*index];
            let task_id = match task.get_id() {
                Some(id) => id,
                None => {
                    let Some(id) = current_task.get_id() else {
                        return Err(TaskManagerError::MissingTaskId(current_task.title.clone()).into());
                    };
                    task.set_id(id);
                    id
                }
            };

            self.db.borrow_mut().update_task(&task)?;
            if self.show_only_tagless {
                if task.tags.is_empty() {
                    self.tasks[*index] = task;
                    self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
                } else { self.tasks.remove(*index); }
                return Ok(());
            }

            let passes_tags_filter = self.db.borrow_mut().task_passes_tag_filter(task_id, &self.tags_filter)?;
            let passes_sql_filter = match self.sql_filter {
                None => true,
                Some(ref filter) => self.db.borrow_mut().task_passes_custom_filter(task_id, filter)?
            };
            let passes_hide_done = self.task_passes_hide_done_filter(&task);

            if passes_tags_filter && passes_sql_filter && passes_hide_done {
                self.tasks[*index] = task;
                self.tasks.sort_by(|a, b| self.current_ordering.compare_tasks(a, b));
            } else {
                self.tasks.remove(*index);
            }
        } else {
            self.update_task_with_id(task)?;
        }

        Ok(())
    }

    pub fn update_task_with_id(&mut self, task: Task) -> Result<()> {
        let Some(task_id) = task.get_id() else {
            return Err(TaskManagerError::MissingTaskId(task.title).into());
        };

        let index = self.tasks.iter().position(|task| {
            let Some(id) = task.get_id() else { return false; };
            task_id == id
        });
        self.update_task(task, &index)?;
        Ok(())
    }

    pub fn add_tag(&mut self, tag: &mut Tag) -> Result<()> {
        self.db.borrow_mut().add_tag(tag)?;
        self.tags.push(tag.clone());
        self.tags.sort_by(|a, b| a.text.cmp(&b.text));
        Ok(())
    }

    pub fn update_tag(&mut self, tag: Tag) -> Result<()> {
        let Some(tag_id) = tag.get_id() else {
            return Err(TaskManagerError::MissingTagId(tag.text).into());
        };

        for current_tag in self.tags.iter_mut() {
            if let Some(current_id) = current_tag.get_id() {
                if current_id == tag_id {
                    self.db.borrow_mut().update_tag(&tag)?;
                    *current_tag = tag;
                    break;
                }
            }
        }
        self.tags.sort_by(|a, b| a.text.cmp(&b.text));
        Ok(())
    }

    pub fn delete_tag(&mut self, tag_id: i64) -> Result<()> {
        self.db.borrow_mut().delete_tag(tag_id)?;
        self.tags_filter.retain(|&id| id != tag_id);
        self.update()?;
        Ok(())
    }
}

#[cfg(test)]
mod task_manager_test {
    use super::*;
    use crate::utils::status::Status;
    use chrono::{
        Utc, TimeDelta
    };
        
    #[test]
    fn test() -> Result<()> {
        let db = Rc::new(RefCell::new(Database::new(None)?));
        let mut taskmanager = TaskManager::new(Rc::clone(&db));

        db.borrow_mut().add_tag(&mut Tag::new(None, "first Tag"))?; // id 1
        db.borrow_mut().add_tag(&mut Tag::new(None, "second Tag"))?; // id 2
        db.borrow_mut().add_tag(&mut Tag::new(None, "third Tag"))?; // id 3
        db.borrow_mut().add_tag(&mut Tag::new(None, "tag no 4"))?; // id 4
        db.borrow_mut().add_tag(&mut Tag::new(None, "my tag no 5"))?; // id 5

        db.borrow_mut().add_task(&mut Task::new( // id 1
                None, 
                "my title", 
                "my description", 
                1, 
                vec![1, 4], 
                Status::Pending, 
                Utc::now().naive_utc(), 
                None, 
                None, 
                None, 
                None
        ))?;

        db.borrow_mut().add_task(&mut Task::new( // id 2
                None, 
                "my second title", 
                "my 2 description", 
                2, 
                vec![1, 2, 3, 4], 
                Status::Pending, 
                Utc::now().naive_utc(), 
                Some(Utc::now().naive_utc() + TimeDelta::days(5)), 
                Some(Utc::now().naive_utc() + TimeDelta::days(3)),
                Some(Utc::now().naive_utc() + TimeDelta::seconds(5)),
                Some(TimeDelta::minutes(420))
        ))?;

        db.borrow_mut().add_task(&mut Task::new( // id 3
                None, 
                "my third title", 
                "my 3 description", 
                3, 
                vec![1, 2, 3], 
                Status::Pending, 
                Utc::now().naive_utc(), 
                None, 
                None,
                None, 
                None
        ))?;

        taskmanager.update()?;

        taskmanager.update_tag(Tag::new(Some(4), "hi"))?;

        taskmanager.add_task(Task::new( // id 4
                None, 
                "task number 4", 
                "four me", 
                2, 
                vec![3,1,4], 
                Status::InProgress, 
                Utc::now().naive_utc(), 
                None, 
                None,
                None, 
                None
        ))?;

        assert_eq!(taskmanager.get_tasks().len(), 4);
        assert!(taskmanager.get_tasks().iter().any(|task| task.title == "my title"));
        assert!(taskmanager.get_tasks().iter().any(|task| task.title == "my second title"));
        assert!(taskmanager.get_tasks().iter().any(|task| task.title == "my third title"));
        assert!(taskmanager.get_tasks().iter().any(|task| task.title == "task number 4"));

        taskmanager.set_filter(vec![], Some("status != '\"Done\"'"))?; 
        let mut task = taskmanager.get_tasks()[3].clone();
        task.status = Status::Done;
        // task.status = Status::Pending;
        taskmanager.update_task(task, &Some(3))?;
        Ok(())
    }
}


