use core::fmt;

use serde::{
    Serialize, Deserialize
};
use chrono::{
    Datelike, Days, Months, NaiveDateTime, TimeDelta 
};
use anyhow::Result;

use crate::errors::EventManagerError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceInterval {
    EveryNDays(u32), 
    EveryWeekday(Vec<chrono::Weekday>),
    EveryStartOfMonth,
    EveryEndOfMonth,
    EveryYear,
    EveryNWeeks(u32),
}

impl fmt::Display for RecurrenceInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecurrenceInterval::EveryNDays(n) => {
                if *n > 1 {
                    write!(f, "every {} days", n)
                } else { 
                    write!(f, "every {} day", n)
                }
            },
            RecurrenceInterval::EveryWeekday(weekday) => {
                let days = weekday
                    .iter()
                    .map(|d| format!("{:?}", d)) // Format properly
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "every {}", days)
            },
            RecurrenceInterval::EveryStartOfMonth => {
                write!(f, "every start of month")
            },
            RecurrenceInterval::EveryEndOfMonth => {
                write!(f, "every end of month")
            },
            RecurrenceInterval::EveryYear => {
                write!(f, "every year")
            },
            RecurrenceInterval::EveryNWeeks(n) => {
                if *n > 1 {
                    write!(f, "every {} weeks", n)
                } else {
                    write!(f, "every week")
                }
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceDeletion {
    All,
    FromHere(NaiveDateTime),
    ThisOnly(NaiveDateTime),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    id: Option<i64>,

    pub title: String,
    pub description: String,

    pub duration: Option<TimeDelta>,
    pub date: NaiveDateTime,

    pub recurrence_interval: Option<RecurrenceInterval>,
    pub recurrence_end: Option<NaiveDateTime>,
    pub skipped_dates: Vec<NaiveDateTime>
}

impl Event {
    pub fn new(
        id: Option<i64>, 
        title: &str, 
        description: &str, 
        duration: Option<TimeDelta>, 
        date: NaiveDateTime,
        recurrence_interval: Option<RecurrenceInterval>,
        recurrence_end: Option<NaiveDateTime>,
        skipped_dates: Vec<NaiveDateTime>
        ) -> Self {
        Self { 
            id, 
            title: title.to_string(),
            description: description.to_string(),
            duration,
            date,
            recurrence_interval,
            recurrence_end,
            skipped_dates
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
            panic!("Cannot set id for Event: {:?} that already has one, current id: {:?}", self.title, self.id);
        }
    }

    pub fn update_date(&mut self) -> Result<()> {
        let Some(ref recurrence_interval) = self.recurrence_interval else { return Ok(()); };
        match recurrence_interval {
            RecurrenceInterval::EveryWeekday(weekdays) => {
                // go to upcoming closest weekday
                let weekday = weekdays.iter()
                    .min_by_key(|w| {
                        let mut diff = w.number_from_monday() as i8 - self.date.weekday().number_from_monday() as i8;
                        if diff < 0 {
                            diff += 7;
                        }
                        diff
                    });

                let Some(weekday) = weekday else { return Ok(()); };
                let diff = match weekday.number_from_monday() >= self.date.weekday().number_from_monday() {
                    true => weekday.number_from_monday() - self.date.weekday().number_from_monday(),
                    false => (weekday.number_from_monday() + 7) - self.date.weekday().number_from_monday()
                };
                self.date = self.date.checked_add_days(Days::new(diff as u64))
                    .ok_or_else(|| EventManagerError::DateOverflow(1, self.date))?;
                }

            RecurrenceInterval::EveryStartOfMonth => {
                if self.date.day() != 1 {
                    self.date = self.date
                        .checked_add_months(Months::new(1))
                        .ok_or_else(|| EventManagerError::DateOverflow(1, self.date))?
                        .with_day(1)
                        .ok_or_else(|| EventManagerError::InvalidDate(self.date.year(), self.date.month(), 1))?;
                }            
            }

            RecurrenceInterval::EveryEndOfMonth => {
                let next_day = self.date.date().checked_add_days(Days::new(1))
                    .ok_or_else(|| EventManagerError::DateOverflow(1, self.date))?;

                if next_day.day() != 1 {
                    // move to end of this month
                    let start_of_next_month = self.date
                        .checked_add_months(Months::new(1))
                        .ok_or_else(|| EventManagerError::DateOverflow(1, self.date))?
                        .with_day(1)
                        .ok_or_else(|| EventManagerError::InvalidDate(self.date.year(), self.date.month(), 1))?;

                    let end_of_this_month = start_of_next_month
                        .checked_sub_days(Days::new(1))
                        .ok_or_else(|| EventManagerError::DateOverflow(1, start_of_next_month))?;

                    self.date = NaiveDateTime::new(end_of_this_month.date(), self.date.time());
                }
            }

            _ => ()
        }

        Ok(())
    }

    pub fn is_event_occurring_on_date(&self, datetime: &NaiveDateTime) -> Option<NaiveDateTime> {
        if self.recurrence_interval.is_none() {
            if self.date.year() == datetime.year() && self.date.month() == datetime.month() && self.date.day() == datetime.day() {
                return Some(self.date);
            } else {
                return None;
            }
        }

        if let Some(recurrence_interval) = &self.recurrence_interval {
            if let Some(recurrence_end) = self.recurrence_end {
                if *datetime >= recurrence_end {
                    return None; // the event doesn't occur after the end date
                }
            }

            for skipped_date in self.skipped_dates.iter() {
                let same_year = skipped_date.year() == datetime.year();
                let same_month = skipped_date.month() == datetime.month();
                let same_day = skipped_date.day() == datetime.day();
                if same_year && same_month && same_day {
                    return None; // event is skipped on this date
                }
            }

            match recurrence_interval {
                RecurrenceInterval::EveryNDays(n) => {
                    let days_diff = (datetime.date() - self.date.date()).num_days();
                    if days_diff >= 0 && days_diff % *n as i64 == 0 {
                        return Some(self.date.checked_add_days(Days::new(days_diff as u64)).unwrap());
                    }
                }

                RecurrenceInterval::EveryWeekday(weekdays) => {
                    if weekdays.contains(&datetime.weekday()) {
                        let days_diff = (datetime.date() - self.date.date()).num_days();
                        if days_diff >= 0 {
                            return Some(self.date.checked_add_days(Days::new(days_diff as u64)).unwrap());
                        }
                    }
                }

                RecurrenceInterval::EveryStartOfMonth => {
                    if datetime.day() == 1 {
                        let days_diff = (datetime.date() - self.date.date()).num_days();
                        if days_diff >= 0 {
                            return Some(NaiveDateTime::new(datetime.date(), self.date.time()));
                        }
                    }
                }

                RecurrenceInterval::EveryEndOfMonth => {
                    let is_last_day_of_month = match datetime.checked_add_days(Days::new(1)) {
                        Some(d) => d.day() == 1,
                        None => false
                    };

                    if datetime.day() == datetime.day() && is_last_day_of_month {
                        let days_diff = (datetime.date() - self.date.date()).num_days();
                        if days_diff >= 0 {
                            return Some(NaiveDateTime::new(datetime.date(), self.date.time()));
                        }
                    }
                }

                RecurrenceInterval::EveryYear => {
                    if datetime.year() >= self.date.year() {
                        if self.date.month() == 2 && self.date.day() == 29 {
                            // special case: original date is feb 29
                            if datetime.month() == 2 && (datetime.day() == 28 || datetime.day() == 29) {
                                return Some(NaiveDateTime::new(datetime.date(), self.date.time()));
                            }
                        } else if datetime.month() == self.date.month() && datetime.day() == self.date.day() {
                            return Some(NaiveDateTime::new(datetime.date(), self.date.time()));
                        }
                    }
                }

                RecurrenceInterval::EveryNWeeks(n) => {
                    let days_diff = (datetime.date() - self.date.date()).num_days();
                    if days_diff >= 0 && days_diff % (*n as i64 * 7) == 0 {
                        return Some(self.date.checked_add_days(Days::new(days_diff as u64)).unwrap());
                    }
                }
            }
        }

        None
    }
}

