use std::{ cell::RefCell, rc::Rc };
use anyhow::Result;

use chrono::{Days, NaiveDateTime, NaiveTime};

use crate::{errors::EventManagerError, utils::{
    database::Database, event::{Event, RecurrenceDeletion}
}};

pub struct EventManager {
    db: Rc<RefCell<Database>>,
    events: Vec<Event>
}

impl EventManager {
    pub fn new(db: Rc<RefCell<Database>>) -> Self {
        Self {
            db,
            events: vec![]
        }
    }

    pub fn get_events(&self) -> &Vec<Event> {
        &self.events
    }

    pub fn get_events_for_date(&self, date_time: &NaiveDateTime) -> Result<Vec<(usize, &Event, NaiveDateTime)>> {
        let mut result: Vec<(usize, &Event, NaiveDateTime)> = vec![];

        let time = NaiveTime::from_hms_opt(0, 0, 0).ok_or_else(|| EventManagerError::InvalidTime(0, 0, 0))?;
        let date = NaiveDateTime::new(date_time.date(), time);
        let next_day = date.checked_add_days(Days::new(1));
            // .ok_or_else(|| EventManagerError::DateOverflow(1, date))?;

        // find first index where all dates are >= date
        let index = self.events.binary_search_by(|e| {
            if e.date < date {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }).unwrap_or_else(|i| i);

        let mut sort_needed = false;
        for (i, event) in self.events.iter().enumerate() {
            if let Some(d) = next_day {
                if event.date >= d { break; }
            }

            if i >= index && event.recurrence_interval.is_none() { // thus self.events[i] >= date
                result.push((i, &event, event.date)); 
                continue;
            }

            // treat recurring events
            let Some(event_date) = event.is_event_occurring_on_date(&date) else { continue; };
            sort_needed = true;
            result.push((i, event, event_date)); 
        }

        // sort by date
        if sort_needed {
            result.sort_by_key(|entry| entry.2); 
        }

        Ok(result)
    }

    pub fn update(&mut self) -> Result<()> {
        self.events = self.db.borrow_mut().get_events()?;
        Ok(())
    }

    pub fn delete_event(&mut self, event_index: usize, deletion_option: Option<RecurrenceDeletion>) -> Result<()> { 
        let event_to_delete = &self.events[event_index];
        let Some(id) = event_to_delete.get_id() else {
            return Err(EventManagerError::MissingEventId(event_to_delete.title.clone()).into());
        };

        let mut event_to_delete = event_to_delete.clone();

        match deletion_option {
            Some(RecurrenceDeletion::FromHere(date)) => {
                // just update the events recurring end date
                let time = NaiveTime::from_hms_opt(0, 0, 0).ok_or_else(|| EventManagerError::InvalidTime(0, 0, 0))?;
                let recurring_end_date = NaiveDateTime::new(date.date(), time);
                event_to_delete.recurrence_end = Some(recurring_end_date);
                self.update_event(event_to_delete, &Some(event_index))?;
            }

            Some(RecurrenceDeletion::ThisOnly(date)) => {
                // just update the events end recurring date
                event_to_delete.skipped_dates.push(date);
                self.update_event(event_to_delete, &Some(event_index))?;
            }

            _ => { // its a non recurring event or the user wants to delete All
                self.db.borrow_mut().delete_event(id)?;
                self.events.remove(event_index); 
            }
        }
        Ok(())
    }

    pub fn delete_event_with_id(&mut self, event: Event, deletion_option: Option<RecurrenceDeletion>) -> Result<()> {
        let Some(event_id) = event.get_id() else {
            return Err(EventManagerError::MissingEventId(event.title.clone()).into());
        };

        let index = self.events.iter().position(|event| {
            let Some(id) = event.get_id() else { return false; };
            event_id == id
        }).ok_or_else(|| EventManagerError::EventIdNotFound(event_id))?;

        self.delete_event(index, deletion_option)?;
        Ok(())
    }

    pub fn add_event(&mut self, mut event: Event) -> Result<usize> {
        self.db.borrow_mut().add_event(&mut event)?;
        let idx = self.events.binary_search_by(|e| e.date.cmp(&event.date)).unwrap_or_else(|i| i); 
        self.events.insert(idx, event);
        Ok(idx)
    }

    pub fn update_event(&mut self, mut event: Event, event_index: &Option<usize>) -> Result<()> {
        if let Some(index) = event_index {
            let current_event = &self.events[*index];
            if event.get_id().is_none() {
                let Some(current_event_id) = current_event.get_id() else {
                    return Err(EventManagerError::MissingEventId(current_event.title.clone()).into());
                };

                event.set_id(current_event_id);
            }
            self.db.borrow_mut().update_event(&event)?;
            let sort_needed = self.events[*index].date != event.date;
            self.events[*index] = event;
            if sort_needed {
                self.events.sort_by_key(|e| e.date);
            }
        } else {
            self.update_event_with_id(event)?;
        }

        Ok(())
    }

    pub fn update_event_with_id(&mut self, event: Event) -> Result<()> {
        let Some(event_id) = event.get_id() else {
            return Err(EventManagerError::MissingEventId(event.title.clone()).into());
        };

        let index = self.events.iter().position(|event| {
            let Some(id) = event.get_id() else { return false; };
            event_id == id
        });
        self.update_event(event, &index)?;
        Ok(())
    }
}

#[cfg(test)]
mod event_manager_test {
    use super::*;
    use crate::utils::event::RecurrenceInterval;
    use chrono::{
        Utc, TimeDelta
    };
        
    #[test]
    fn test() -> Result<()> {
        let db = Rc::new(RefCell::new(Database::new(None)?));
        let mut eventmanager = EventManager::new(Rc::clone(&db));

        eventmanager.add_event(Event::new( // id 1
                None, 
                "my first event", 
                "my one description", 
                Some(TimeDelta::minutes(45)),
                Utc::now().naive_local(),
                Some(RecurrenceInterval::EveryWeekday(vec![chrono::Weekday::Sun])),
                None,
                vec![]
        ))?;

        eventmanager.add_event(Event::new( // id 2
                None, 
                "some second event", 
                "my seconds descr", 
                Some(TimeDelta::minutes(90)),
                Utc::now().naive_local(),
                Some(RecurrenceInterval::EveryWeekday(vec![chrono::Weekday::Wed])),
                None,
                vec![]
        ))?;

        eventmanager.add_event(Event::new( // id 3
                None, 
                "some third event", 
                "my three descr", 
                Some(TimeDelta::minutes(90)),
                Utc::now().naive_local(),
                Some(RecurrenceInterval::EveryNDays(2)),
                None,
                vec![]
        ))?;

        eventmanager.add_event(Event::new( // id 4
                None, 
                "some number 4 event", 
                "my 4 descr", 
                Some(TimeDelta::minutes(90)),
                Utc::now().naive_local(),
                None,
                None,
                vec![]
        ))?;

        assert_eq!(eventmanager.get_events().len(), 4);

        let mut second_event = eventmanager.get_events()[1].clone();
        let new_title = "my new title".to_string();
        second_event.title = new_title.clone();
        eventmanager.update_event(second_event, &Some(1))?;
        let second_event = &eventmanager.get_events()[1];
        assert_eq!(second_event.title, new_title);

        Ok(())
    }
}


