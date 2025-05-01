use chrono::{Days, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use rand::{seq::IteratorRandom, Rng};
use anyhow::Result;
use rusqlite::{
    named_params, params, Connection, ToSql
};
use crate::errors::DataBaseError;

use super::{
    event::{Event, RecurrenceInterval}, status::Status, tag::Tag, task::Task
};


pub struct Database {
    db: Connection
}

impl Database {
    pub fn new(path: Option<&str>) -> Result<Self> {
        let db = match path {
            None => Connection::open_in_memory()?,
            Some(path) => Connection::open(path)?
        };

        let mut needed_table_names: Vec<&str> = vec!["tasks", "events", "tags", "task_tags", "task_view"];
        {
           // check if table names are already available
            let sql_query = "SELECT name FROM sqlite_master WHERE type = 'table';";
            let mut stmt = db.prepare(sql_query)?;
            let table_names = stmt.query_map([], |row| {
                Ok(row.get::<_, String>(0)?)
            })?;

            for table_name in table_names {
                let name: String = table_name?;
                needed_table_names.retain(|&x| x != name.as_str());
            }
        }
           
        let mut table_creation_queries: Vec<&str> = vec![];

        for table_name in needed_table_names {
            match table_name {
                "tasks" => table_creation_queries.push("
                    CREATE TABLE tasks (
                        id INTEGER PRIMARY KEY NOT NULL,
                        title TEXT,
                        description TEXT,
                        severity INTEGER DEFAULT 0,
                        status TEXT NOT NULL,
                        creation_date TEXT NOT NULL,
                        due_date TEXT,
                        reminder_date TEXT,
                        timer_start TEXT,
                        time_taken TEXT
                    );"),
                "events" => table_creation_queries.push("
                    CREATE TABLE events (                                                               
                        id INTEGER PRIMARY KEY NOT NULL,                                               
                        title TEXT,
                        description TEXT,
                        duration TEXT,
                        date TEXT,
                        recurrence_interval TEXT,
                        recurrence_end TEXT,
                        skipped_dates TEXT
                    );"),
                "tags" => table_creation_queries.push("
                    CREATE TABLE tags (
                        id INTEGER PRIMARY KEY NOT NULL,
                        text TEXT UNIQUE
                    );"),
                "task_tags" => table_creation_queries.push("
                    CREATE TABLE task_tags (
                        task_id INTEGER,
                        tag_id INTEGER,
                        FOREIGN KEY (task_id) REFERENCES tasks(id) ON UPDATE CASCADE ON DELETE CASCADE,
                        FOREIGN KEY (tag_id) REFERENCES tags(id) ON UPDATE CASCADE ON DELETE CASCADE,
                        PRIMARY KEY (task_id, tag_id)
                    );"),
                "task_view" => {
                    table_creation_queries.push("
                        DROP VIEW IF EXISTS task_view;
                    ");
                    table_creation_queries.push("
                        CREATE VIEW task_view AS SELECT 
                            t.id,
                            t.title,
                            t.description,
                            t.severity,
                            t.status,
                            t.creation_date,
                            t.due_date,
                            t.reminder_date,
                            t.timer_start,
                            t.time_taken,
                            printf('%02d', (json_extract(t.time_taken, '$[0]')/(3600))) || ':' ||
                            printf('%02d', ((json_extract(t.time_taken, '$[0]')%3600) / 60)) || ':' ||
                            printf('%02d', json_extract(t.time_taken, '$[0]') % 60) AS time_taken_readable,
                            '[' || IFNULL(GROUP_CONCAT('\"' || tag.text || '\"', ','), '') || ']' AS tags,
                            '[' || IFNULL(GROUP_CONCAT(tag.id, ','), '') || ']' AS tag_ids
                        FROM 
                            tasks t
                        LEFT JOIN 
                            task_tags tt ON t.id = tt.task_id
                        LEFT JOIN 
                            tags tag ON tt.tag_id = tag.id
                        GROUP BY 
                            t.id;
                        ");
                },
                _ => ()
           }
        }

        for query in table_creation_queries {
            db.execute(query, [])?;
        }

        db.pragma_update(None, "foreign_keys", 1)?;
        let foreign_keys_value = db.pragma_query_value(None, "foreign_keys", |row| { 
            Ok(row.get::<_, u8>(0)?)
        })?;

        if foreign_keys_value != 1 { 
            return Err(DataBaseError::ForeignKeyUpdateFailed(foreign_keys_value).into());
        }

        Ok(Self { db })
    }

    fn from_json_cell<T: serde::de::DeserializeOwned>(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<T> {
        let json_str: String = row.get(idx)?;
        serde_json::from_str(&json_str)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e)))
    }

    pub fn fill_database(&mut self) -> Result<()> {
        let tag_id_amount = 6;

        let tags: Vec<String> = (0..tag_id_amount).map(|x| {
            let mut tag_str = "tag ".to_string();
            tag_str.push_str(&x.to_string());
            tag_str
        }).collect();

        for tag_string in tags.iter() {
            self.add_tag(&mut Tag::new(None, tag_string))?;
        }

        let mut rng = rand::rng();

        let mut tasks: Vec<Task> = (0..300).map(|x| {
            let title = format!("task number {}", x);
            let description = format!("description number {}", x);

            let amount = (0..tag_id_amount).choose(&mut rng).unwrap_or_else(|| 0);
            let tag_ids = (1..=tag_id_amount).choose_multiple(&mut rng, amount).iter().map(|&x| x as i64).collect();

            let status: Status = match rng.random_range(0..3) {
                0 => Status::InProgress,
                1 => Status::Done,
                _ => Status::Pending,
            };

            let seconds_duration = rng.random_range(0..5*60*60);

            let task = Task::new(
                None, 
                title.as_str(), 
                description.as_str(), 
                x, 
                tag_ids, 
                status, 
                Utc::now().naive_utc(), 
                Some(Utc::now().naive_utc() + Duration::hours(5)), 
                Some(Utc::now().naive_utc() + Duration::hours(3)), 
                None, 
                Some(TimeDelta::seconds(seconds_duration))
                );
            task
        }).collect();

        for task in tasks.iter_mut() {
            self.add_task(task)?;
        }

        let mut events: Vec<Event> = (0..30).map(|x| {
            let title = format!("event number {}", x);
            let description = format!("description number {}", x);

            let has_duration = rng.random_range(0..=1) == 1;
            let seconds_duration = rng.random_range(0..180*60);
            let duration = if has_duration {
                TimeDelta::new(seconds_duration, 0)
            } else { None };


            let rec_interval = rng.random_range(0..=3);
            let recurrence_interval = if rec_interval == 0 {
                let n = rng.random_range(1..5);
                Some(RecurrenceInterval::EveryNDays(n))
            } else if rec_interval == 1 {
                Some(super::event::RecurrenceInterval::EveryWeekday(vec![chrono::Weekday::Mon]))
            } else if rec_interval == 2 {
                Some(super::event::RecurrenceInterval::EveryWeekday(vec![chrono::Weekday::Wed, chrono::Weekday::Fri]))
            } else { None };

            let times = [
                NaiveTime::from_hms_opt(8, 45, 0).expect("invalid time: 08:45:00"),
                NaiveTime::from_hms_opt(9, 0, 0).expect("invalid time: 09:00:00"),
                NaiveTime::from_hms_opt(14, 30, 0).expect("invalid time: 14:30:00"),
                NaiveTime::from_hms_opt(16, 45, 0).expect("invalid time: 16:45:00"),
                NaiveTime::from_hms_opt(15, 15, 0).expect("invalid time: 15:15:00"),
                NaiveTime::from_hms_opt(7, 0, 0).expect("invalid time: 07:00:00"),
                NaiveTime::from_hms_opt(23, 15, 0).expect("invalid time: 23:15:00"),
            ];

            let date = NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2025, 3, 20).expect("invalid date: 2025.3.20") + Days::new(x), 
                times[x as usize % times.len()]);
            let event = Event::new(
                None, 
                title.as_str(), 
                description.as_str(), 
                duration, 
                date,
                recurrence_interval, 
                None, 
                vec![]);
            event
        }).collect();

        for event in events.iter_mut() {
            event.update_date()?;
            self.add_event(event)?;
        }

        Ok(())
    }

    // ------------------------------------ task related ------------------------------------
    pub fn add_task(&self, task: &mut Task) -> Result<()>{ 
        {
            let mut stmt = self.db.prepare("
                INSERT INTO tasks (
                    title,
                    description,
                    severity,
                    status,
                    creation_date,
                    due_date,
                    reminder_date,
                    timer_start,
                    time_taken
                )
                VALUES (
                    :title,
                    :description,
                    :severity,
                    :status,
                    :creation_date,
                    :due_date,
                    :reminder_date,
                    :timer_start,
                    :time_taken);")?;

            stmt.execute(named_params! {
                ":title": task.title,
                ":description": task.description,
                ":severity": task.severity,
                ":status": serde_json::to_string(&task.status)?,
                ":creation_date": serde_json::to_string(&task.creation_date)?,
                ":due_date": serde_json::to_string(&task.due_date)?,
                ":reminder_date": serde_json::to_string(&task.reminder_date)?,
                ":timer_start": serde_json::to_string(&task.timer_start)?,
                ":time_taken": serde_json::to_string(&task.time_taken)?
            })?;
        }

        let current_task_id = self.db.last_insert_rowid();
        task.set_id(current_task_id);

        for &tag_id in task.tags.iter() {
            self.add_tag_to_task(tag_id, current_task_id)?;
        }
        Ok(())
    }

    pub fn update_task(&self, task: &Task) -> Result<()> {
        let task_id = match task.get_id() {
            Some(id) => id,
            None => return Err(DataBaseError::TaskIdMissing(task.title.clone()).into()),
        };

        let sql_query = "
            UPDATE tasks SET 
                title = :title, 
                description = :description,
                severity = :severity,
                status = :status,
                creation_date = :creation_date,
                due_date = :due_date,
                reminder_date = :reminder_date,
                timer_start = :timer_start,
                time_taken = :time_taken
            WHERE id = :id;";

        let params = named_params! {
            ":id": task_id,
            ":title": task.title,
            ":description": task.description,
            ":severity": task.severity,
            ":status": serde_json::to_string(&task.status)?,
            ":creation_date": serde_json::to_string(&task.creation_date)?,
            ":due_date": serde_json::to_string(&task.due_date)?,
            ":reminder_date": serde_json::to_string(&task.reminder_date)?,
            ":timer_start": serde_json::to_string(&task.timer_start)?,
            ":time_taken": serde_json::to_string(&task.time_taken)?
        };

        self.db.execute(&sql_query, params)?;

        // update the tags
        let sql_query = "DELETE FROM task_tags WHERE task_id = ?;";
        self.db.execute(sql_query, params![task.get_id()])?;

        for &tag_id in task.tags.iter() {
            self.add_tag_to_task(tag_id, task_id)?;
        }

        Ok(())
    }

    pub fn delete_task(&self, task_id: i64) -> Result<()> {
        // delete task
        let sql_query = "DELETE FROM tasks WHERE id = ?;";
        self.db.execute(sql_query, params![task_id])?;
        Ok(())
    }

    pub fn delete_done_tasks(&self) -> Result<()> {
        let sql_query = "DELETE FROM tasks WHERE status == '\"Done\"';";
        self.db.execute(sql_query, params![])?;
        Ok(())
    }
    
    fn add_tag_to_task(&self, tag_id: i64, task_id: i64) -> Result<()> {
        let mut stmt = self.db.prepare(" 
            INSERT INTO task_tags (task_id, tag_id)
            VALUES (:task_id, :tag_id);")?;

        stmt.execute(named_params!{ ":task_id": task_id, ":tag_id": tag_id })?; 
        Ok(())
    }

    // ------------------------------------ filter related ------------------------------------
    pub fn task_passes_custom_filter(&self, task_id: i64, custom_filter: &str) -> Result<bool> {
        let sql_query = format!("
                SELECT EXISTS (
                    SELECT 1 FROM tasks WHERE id = :id 
                    AND ({}));", custom_filter);

        let params = named_params! {
            ":id": task_id
        };

        let mut stmt = self.db.prepare(&sql_query)?;
        let res = stmt.query_row(params, |row| {
            Ok(row.get::<_, u8>(0)?) 
        })?;

        Ok(res == 1)
    } 

    pub fn task_passes_tag_filter(&self, task_id: i64, filtered_tags: &Vec<i64>) -> Result<bool> { 
        let tags_len = filtered_tags.len();
        if tags_len == 0 { return Ok(true); }

        let mut param_values: Vec<&dyn ToSql> = Vec::new();
        param_values.push(&task_id as &dyn ToSql);

        for val in filtered_tags.iter() {
            param_values.push(val as &dyn ToSql);
        }
        param_values.push(&tags_len as &dyn ToSql);

        let placeholders = filtered_tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql_query = format!("
                    SELECT EXISTS (
                        SELECT 1
                        FROM task_tags
                        WHERE task_id = ? 
                        AND tag_id IN ({})
                        GROUP BY task_id
                    HAVING COUNT(DISTINCT tag_id) = ?);", 
                    placeholders.to_string());

        let mut stmt = self.db.prepare(&sql_query)?;

        let res = stmt.query_row(&*param_values, |row| { Ok(row.get::<_, u8>(0)?) })?;
        Ok(res == 1)
    }

    pub fn get_tags_for_task(&self, task_id: i64) -> Result<Vec<Tag>> { 
        let sql_query = "
            SELECT 
                tags, 
                tag_ids
            FROM task_view
            WHERE id = :id;";

        let (text_array, id_array) = self.db.query_row(sql_query, 
            named_params! { ":id": task_id }, |row| { 
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)) 
            })?;

        let text_vec: Vec<String> = serde_json::from_str(&text_array)?;
        let tag_id_vec: Vec<i64> = serde_json::from_str(&id_array)?;

        let mut tag_vec: Vec<Tag> = vec![];
        for (text, id) in text_vec.iter().zip(tag_id_vec.iter()) {
            tag_vec.push(Tag::new(Some(*id), text));
        }

        Ok(tag_vec)
    }

    pub fn get_filtered_tasks(&self, filtered_tags: &Vec<i64>, custom_filter: Option<&str>) -> Result<Vec<Task>> {
        let mut sql_query = "
            SELECT 
                id,
                title,
                description,
                severity,
                tag_ids,
                status,
                creation_date,
                due_date,
                reminder_date,
                timer_start,
                time_taken
            FROM task_view".to_string(); 
        
        let mut param_values: Vec<&dyn ToSql> = Vec::new();
        
        let tags_len = filtered_tags.len();
        if tags_len > 0 {
            let placeholders = filtered_tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql_query.push_str(&format!(" WHERE id in (
                    SELECT task_id FROM task_tags
                        WHERE tag_id IN ({})
                        GROUP BY task_id
                        HAVING COUNT(DISTINCT tag_id) = ?)", placeholders));

            for val in filtered_tags.iter() {
                param_values.push(val as &dyn ToSql);
            }
            param_values.push(&tags_len as &dyn ToSql);
        }

        match custom_filter {
            None => (),
            Some(custom_filter) => {
                if tags_len > 0 {
                    sql_query.push_str(&format!(" AND ({});", custom_filter));
                } else {
                    sql_query.push_str(&format!(" WHERE ({});", custom_filter));
                }
            }
        }

        let mut stmt = self.db.prepare(&sql_query)?;
        let tasks_iter = stmt.query_map(&*param_values, |row| {
            let title: String = row.get(1)?;
            let description: String = row.get(2)?;
            let severity: i64 = row.get(3)?;
            let task = Task::new(
                row.get(0)?,
                title.as_str(),
                description.as_str(),
                severity,
                Self::from_json_cell(row, 4)?,
                Self::from_json_cell(row, 5)?,
                Self::from_json_cell(row, 6)?,
                Self::from_json_cell(row, 7)?,
                Self::from_json_cell(row, 8)?,
                Self::from_json_cell(row, 9)?,
                Self::from_json_cell(row, 10)?
            );

            Ok(task)
        })?;
    

        let mut tasks: Vec<Task> = vec![];
        for task in tasks_iter {
            tasks.push(task?);
        }

        Ok(tasks)
    }

    pub fn get_tracked_tasks(&self) -> Result<Vec<i64>> {
        let sql_query = "
            SELECT id FROM task_view WHERE timer_start IS NOT 'null';".to_string(); 
        
        let param_values: Vec<&dyn ToSql> = Vec::new();

        let mut stmt = self.db.prepare(&sql_query)?;
        let task_ids_iter = stmt.query_map(&*param_values, |row| {
            let task_id: Option<i64> = row.get(0)?;
            Ok(task_id)
        })?;
    

        let mut ids: Vec<i64> = vec![];
        for task_id in task_ids_iter {
            let Some(id) = task_id? else {
                return Err(DataBaseError::TaskIdMissing("NO NAME".to_string()).into());
            };
            ids.push(id);
        }

        Ok(ids)
    }

    pub fn get_task_from_id(&self, id: i64) -> Result<Task> {
        let sql_query = "
            SELECT 
            id,
            title,
            description,
            severity,
            tag_ids,
            status,
            creation_date,
            due_date,
            reminder_date,
            timer_start,
            time_taken
                FROM task_view WHERE id = :id;".to_string(); 

        let params = named_params! {
            ":id": id
        };

        let mut stmt = self.db.prepare(&sql_query)?;

        let res = stmt.query_row(params, |row| {
            let title: String = row.get(1)?;
            let description: String = row.get(2)?;
            let severity: i64 = row.get(3)?;
            let task = Task::new(
                row.get(0)?,
                title.as_str(),
                description.as_str(),
                severity,
                Self::from_json_cell(row, 4)?,
                Self::from_json_cell(row, 5)?,
                Self::from_json_cell(row, 6)?,
                Self::from_json_cell(row, 7)?,
                Self::from_json_cell(row, 8)?,
                Self::from_json_cell(row, 9)?,
                Self::from_json_cell(row, 10)?
            );

            Ok(task)
        })?;

        Ok(res)
    }

    // ------------------------------------ tag related ------------------------------------
    pub fn add_tag(&self, tag: &mut Tag) -> Result<()> {
        let sql_query = " INSERT INTO tags (text) VALUES (:text);";
        let mut stmt = self.db.prepare(sql_query)?;
        stmt.execute(named_params!{ ":text": tag.text })?; 

        let curren_tag_id = self.db.last_insert_rowid();
        tag.set_id(curren_tag_id);
        Ok(())
    }

    pub fn update_tag(&self, tag: &Tag) -> Result<()> {
        let tag_id = match tag.get_id() {
            Some(id) => id,
            None => return Err(DataBaseError::TagIdMissing(tag.text.clone()).into()),
        };

        let sql_query = "
            UPDATE tags SET
                text = :text
            WHERE id = :id;";

        let params = named_params! {
            ":id": tag_id,
            ":text": tag.text
        };

        self.db.execute(sql_query, params)?;
        Ok(())
    }

    pub fn get_id_from_tag_text(&self, text: &str) -> Result<i64> {
        let sql_query = " SELECT id FROM tags WHERE text = :text;";
        let res = self.db.query_row(sql_query, [text], |row| { Ok(row.get::<_, i64>(0)?) })?;
        Ok(res)
    }

    pub fn delete_tag(&self, tag_id: i64) -> Result<()> {
        let sql_query = "DELETE FROM tags WHERE id = ?;";
        let _ = self.db.execute(&sql_query, params![tag_id])?;
        Ok(())
    }

    pub fn get_tags(&self) -> Result<Vec<Tag>> {
        let sql_query = "SELECT id, text FROM tags;";
        let mut stmt = self.db.prepare(sql_query)?;
        let tags_iter = stmt.query_map([], |row| {
            let id = row.get(0)?;
            let text: String = row.get(1)?;
            let tag = Tag::new(id, &text);
            Ok(tag)
        })?;

        let mut tags: Vec<Tag> = vec![];
        for tag in tags_iter {
            tags.push(tag?);
        }

        Ok(tags)
    }

    // ------------------------------------ event related ------------------------------------
    pub fn add_event(&self, event: &mut Event) -> Result<()> { 
        let mut stmt = self.db.prepare("
            INSERT INTO events (
                title,
                description,
                duration,
                date,
                recurrence_interval,
                recurrence_end,
                skipped_dates
            )
            VALUES (
                :title,
                :description,
                :duration,
                :date,
                :recurrence_interval,
                :recurrence_end,
                :skipped_dates);")?;

            stmt.execute(named_params! {
                ":title": event.title,
                ":description": event.description,
                ":duration": serde_json::to_string(&event.duration)?,
                ":date": serde_json::to_string(&event.date)?,
                ":recurrence_interval": serde_json::to_string(&event.recurrence_interval)?,
                ":recurrence_end": serde_json::to_string(&event.recurrence_end)?,
                ":skipped_dates": serde_json::to_string(&event.skipped_dates)?,
            })?;

            let current_event_id = self.db.last_insert_rowid();
            event.set_id(current_event_id);
            Ok(())
    }

    pub fn update_event(&self, event: &Event) -> Result<()> {
        let Some(event_id) = event.get_id() else {
            return Err(DataBaseError::EventIdMissing(event.title.clone()).into());
        };

        let sql_query = "
            UPDATE events SET 
                title = :title,
                description = :description,
                duration = :duration,
                date = :date,
                recurrence_interval = :recurrence_interval,
                recurrence_end = :recurrence_end,
                skipped_dates = :skipped_dates
            WHERE id = :id;";

        let params = named_params! {
            ":id": event_id,
            ":title": event.title,
            ":description": event.description,
            ":duration": serde_json::to_string(&event.duration)?,
            ":date": serde_json::to_string(&event.date)?,
            ":recurrence_interval": serde_json::to_string(&event.recurrence_interval)?,
            ":recurrence_end": serde_json::to_string(&event.recurrence_end)?,
            ":skipped_dates": serde_json::to_string(&event.skipped_dates)?
        };

        self.db.execute(&sql_query, params)?;
        Ok(())
    }

    pub fn delete_event(&self, event_id: i64) ->  Result<()> {
        let sql_query = "DELETE FROM events WHERE id = ?;";
        let _ = self.db.execute(&sql_query, params![event_id])?;
        Ok(())
    }

    pub fn get_events(&self) -> Result<Vec<Event>> {
        let sql_query = "
            SELECT 
                id,
                title,
                description,
                duration,
                date,
                recurrence_interval,
                recurrence_end,
                skipped_dates
            FROM events ORDER BY date ASC".to_string(); 
            let mut stmt = self.db.prepare(&sql_query)?;
        let events_iter = stmt.query_map([], |row| {
            let title: String = row.get(1)?;
            let description: String= row.get(2)?;
            let event = Event::new(
                row.get(0)?,
                title.as_str(), 
                description.as_str(), 
                Self::from_json_cell(row, 3)?,
                Self::from_json_cell(row, 4)?,
                Self::from_json_cell(row, 5)?,
                Self::from_json_cell(row, 6)?,
                Self::from_json_cell(row, 7)?,
            );
            Ok(event)
        })?;

        let mut events: Vec<Event> = vec![];

        for event in events_iter {
            events.push(event?);
        }

        Ok(events)
    }
}

#[cfg(test)]
mod database_test {
    use super::*;
    use chrono::{
        TimeDelta,
        Utc
    };

    use crate::utils::status::Status;

    #[test] 
    fn test_events() -> Result<()> {
        let db = Database::new(None)?;

        db.add_event(&mut Event::new(
                None, 
                "my event", 
                "my description", 
                None, 
                Utc::now().naive_utc(),
                None,
                None,
                vec![]
        ))?;
        
        db.add_event(&mut Event::new(
                None, 
                "my second event", 
                "my 2 descr", 
                None, 
                Utc::now().naive_utc(),
                None,
                None,
                vec![]
        ))?;

        db.add_event(&mut Event::new(
                None, 
                "something else as third", 
                "my third description", 
                Some(TimeDelta::minutes(90)),
                Utc::now().naive_utc(),
                None,
                None,
                vec![]
        ))?;
    
        let events = db.get_events()?;
        assert_eq!(events.len(), 3);
        db.delete_event(events[0].get_id().unwrap())?;
        
        let mut events = db.get_events()?;
        assert_eq!(events.len(), 2);
        let mut new_titles = vec![];
        for (i, event) in events.iter_mut().enumerate() {
            event.title.push_str(&i.to_string());
            new_titles.push(event.title.as_str());
            db.update_event(event)?;
        }

        let events = db.get_events()?;
        assert_eq!(events.len(), 2);
        for event in events.iter() {
            assert!(new_titles.contains(&event.title.as_str()));
            new_titles.retain(|&t| t != event.title.as_str());
        }
        assert_eq!(new_titles.len(), 0);
        Ok(())
    }


    #[test] 
    fn test_tags() -> Result<()> {
        let db = Database::new(None)?;

        let mut tags_to_add = vec!["first tag", "second tag", "third tag", "tag no 4"];

        for tag_text in tags_to_add.iter() {
            db.add_tag(&mut Tag::new(None, tag_text))?;
        }

        let mut tags = db.get_tags()?;
        assert_eq!(tags.len(), 4);

        for tag in tags.iter() {
            assert!(tags_to_add.contains(&tag.text.as_str()));
            tags_to_add.retain(|&t| t != tag.text.as_str());
        }
        assert_eq!(tags_to_add.len(), 0);

        let mut updated_tag_names = vec![];
        for tag in tags.iter_mut() {
            tag.text.push_str(" updated");
            updated_tag_names.push(tag.text.as_str());
            db.update_tag(tag)?;
        }
        
        let tags = db.get_tags()?;
        assert_eq!(tags.len(), 4);

        for tag in tags.iter() {
            assert!(updated_tag_names.contains(&tag.text.as_str()));
            updated_tag_names.retain(|&t| t != tag.text.as_str());
        }
        assert_eq!(updated_tag_names.len(), 0);

        let mut current_tag_ids = vec![];
        for (i, tag) in tags.iter().enumerate() {
            let current_id = tag.get_id().unwrap();
            current_tag_ids.push(current_id);
            assert_eq!(current_id, (i+1) as i64);
        }

        let tag_id_to_delete = vec![2,4];
        for &tag_id in tag_id_to_delete.iter() {
            db.delete_tag(tag_id)?;
        }    
        
        for tag in db.get_tags()?.iter() {
            assert!(!tag_id_to_delete.contains(&tag.get_id().unwrap()));
        }
        Ok(())
    }

    #[test]
    fn test_tasks() -> Result<()> {
        // let db = Database::new(Some("/home/valentin/workspace/example_db"))?;
        let db = Database::new(None)?; // database in ram
        db.add_tag(&mut Tag::new(None, "first Tag"))?; // id 1
        db.add_tag(&mut Tag::new(None, "second Tag"))?; // id 2
        db.add_tag(&mut Tag::new(None, "third Tag"))?; // id 3
        db.add_tag(&mut Tag::new(None, "tag no 4"))?; // id 4
        db.add_tag(&mut Tag::new(None, "my tag no 5"))?; // id 5

        db.add_task(&mut Task::new( // id 1
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

        db.add_task(&mut Task::new( // id 2
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

        db.add_task(&mut Task::new( // id 3
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

        let tasks = db.get_filtered_tasks(&vec![1, 2], None)?;
        let expected_task_ids = vec![2,3];
        for task in tasks.iter() {
            assert!(expected_task_ids.contains(&task.get_id().unwrap()));
        }

        let tasks = db.get_filtered_tasks(&vec![1, 2, 3], None)?;
        let expected_task_ids = vec![2,3];
        for task in tasks.iter() {
            assert!(expected_task_ids.contains(&task.get_id().unwrap()));
        }

        let tasks = db.get_filtered_tasks(&vec![4, 2, 3], None)?;
        let expected_task_ids = vec![2];
        for task in tasks.iter() {
            assert!(expected_task_ids.contains(&task.get_id().unwrap()));
        }

        let mut all_tags = db.get_tags()?;
        let tag_1 = &mut all_tags[0];
        tag_1.text = "new tag name".to_string();
        db.update_tag(&tag_1)?;

        let tasks = db.get_filtered_tasks(&vec![], None)?;
        for task in tasks.iter() {
            assert!(task.tags.contains(&1));
        }

        db.delete_tag(1)?;

        let tasks = db.get_filtered_tasks(&vec![], None)?;
        for task in tasks.iter() {
            assert!(!task.tags.contains(&1));
        }

        db.delete_task(1)?;
        db.delete_task(3)?;

        let tasks = db.get_filtered_tasks(&vec![], None)?;
        for task in tasks.iter() {
            assert!(task.get_id().unwrap() == 2);
        }
        Ok(())
    }
}

