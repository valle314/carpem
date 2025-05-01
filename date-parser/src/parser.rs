use regex::Regex;
use chrono::{Datelike, Days, Duration, Local, Months, NaiveDateTime, NaiveTime};

pub fn parse_date(date_string: &str, mut date: NaiveDateTime) -> Option<NaiveDateTime> {
    // time: 15:45, 16:20, 16:2...
    let Ok(re) = Regex::new(r"^(\d+):(\d+)$") else { return None;};
    if re.is_match(date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(mut hours) = (&cap[1]).parse::<u32>() else { return None; };
            let Ok(mut minutes) = (&cap[2]).parse::<u32>() else { return None; };

            if hours >= 23 { hours = 23; }
            if minutes >= 59 { minutes = 59; }

            let Some(time) = NaiveTime::from_hms_opt(hours, minutes, 0) else { return None; };
            date = NaiveDateTime::new(date.date(), time);
        }
        return Some(date);
    }

    // time: 1545, 1620, ...
    let Ok(re) = Regex::new(r"^(\d{2})(\d{2})$") else { return None;};
    if re.is_match(&date_string)
    {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(mut hours) = (&cap[1]).parse::<u32>() else { return None; };
            let Ok(mut minutes) = (&cap[2]).parse::<u32>() else { return None; };

            if hours >= 23 { hours = 23; }
            if minutes >= 59 { minutes = 59; }

            let Some(time) = NaiveTime::from_hms_opt(hours, minutes, 0) else { return None; };
            date = NaiveDateTime::new(date.date(), time);
        }
        return Some(date);
    }

    // weekday number, fri, fri1, thu2, ..., fri- (current or previous friday), thu-2, ...
    let Ok(re) = Regex::new(r"^(mo|mon|di|tu|tue|mi|we|wed|do|thu|th|fr|fri|sa|sat|su|so|sun)(-{0,1})(\d+|$)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let weekday = regex_to_weekday((&cap[1]).to_string());
            let mut n: u64 = 0;

            if cap[3].len() > 0 {
                n = match (&cap[3]).parse::<u64>() {
                    Ok(value) => value,
                    _ => return None
                };
            }

            let is_negative = cap[2].len() > 0;

            // go to prev, current or incomming weekday
            if date.weekday() != weekday {
                let difference = (weekday.number_from_monday() as i64) - (date.weekday().number_from_monday() as i64);
                date = match (is_negative, difference >= 0) {
                    (false, true) => date + Duration::days(difference),
                    (false, false) => date + Duration::days(difference + 7),
                    (true, true) => date + Duration::days(difference - 7),
                    (true, false) => date + Duration::days(difference)
                };
            }
            date += Duration::days(if is_negative { -7*n as i64 } else { 7*n as i64 });

            return Some(date);
        }
    }

    // day month: 10sep, 12dec (current or the comming one), ...
    let Ok(re) = Regex::new(r"^(\d*)(jan|ja|feb|fe|ma|mä|apr|ap|may|mai|jun|june|juni|jul|july|juli|aug|au|sep|se|okt|oct|oc|ok|nov|no|dec|dez|de)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let mut day: u32= 1;
            if cap[1].len() > 0 {
                day = match (&cap[1]).parse::<u32>() {
                    Ok(value) => value,
                    _ => return None
                }
            }

            let month = regex_to_month((&cap[2]).to_string()).number_from_month();

            let d = date.clone();
            let res = d.with_day(1)
                .and_then(|d| d.with_month(month))
                .and_then(|d| d.with_day(day));

            let Some(res) = res else { return None; };

            if (res - date).num_days() < 0 {
                return res.with_year(res.year() + 1);
            } else { return Some(res); }
        }
    }

    // day month: 21.10, 17.12, 18.5.2024 (current or the comming one), ...
    let Ok(re) = Regex::new(r"^(\d+)\.(\d+)(\.\d+){0,1}$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(day) = (&cap[1]).parse::<u32>() else { return None; };
            let Ok(month) = (&cap[2]).parse::<u32>() else { return None; };

            let mut year: i32 = date.year();

            if !cap.get(3).is_none() {
                if cap[3].len() > 0 {
                    let mut year_string: String = String::from(&cap[3]);
                    year_string.remove(0);
                    year = match year_string.parse::<i32>() {
                        Ok(year)  => year,
                        _ => return None
                    };
                }
            }

            let d = date.clone();
            let res = d.with_day(1)
                .and_then(|d| d.with_month(month))
                .and_then(|d| d.with_year(year))
                .and_then(|d| d.with_day(day));

            let Some(res) = res else { return None; };

            if (res - date).num_days() < 0 && cap.get(3).is_none() {
                return res.with_year(res.year() + 1);
            } else { return Some(res); };
        }
    }

    // 1.w, 24.w (24. calendarweek), ...
    let Ok(re) = Regex::new(r"^(\d+)\.w$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(mut n) = (&cap[1]).parse::<u64>() else { return None; };
            if n == 0 { return None; };
            if n >= 53 { n = 53; }; // we have at max 53 calendarweeks

            // go to the first iso week
            let Some(mut date) = date.with_day(1).and_then(|d| d.with_month(1)) else { return None; };
            let weekday = date.weekday();
            let difference = weekday.number_from_monday() - 1;

            date = match weekday {
                chrono::Weekday::Mon | chrono::Weekday::Tue |
                chrono::Weekday::Wed | chrono::Weekday::Thu => date,
                _ => {
                    match date.checked_add_days(Days::new(7)) {
                        Some(date) => date,
                        _ => return None
                    }
                }
            };

            // go to the current monday 
            // and then go the the n-th calendar week so add n-1 weeks
            return date.checked_sub_days(Days::new(difference as u64))
                .and_then(|d| d.checked_add_days(Days::new(7*(n-1))));
        }
    }

    // 2. (coming 2.*.*), ...
    let Ok(re) = Regex::new(r"^(\d+)\.$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(day) = (&cap[1]).parse::<u32>() else { return None; };

            let res = date.clone();
            let Some(res) = res.with_day(day) else { return None; };

            if (res - date).num_days() < 0 {
                return res.checked_add_months(Months::new(1));
            } else { return Some(res); };

            // return  Some(res);
        }
    }

    // month day: sep10, dec24, (current or the comming one)...
    let Ok(re) = Regex::new(r"^(jan|ja|feb|fe|ma|mä|apr|ap|may|mai|jun|june|juni|jul|july|juli|aug|au|sep|se|okt|oct|oc|ok|nov|no|dec|dez|de)(\d*)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let mut day: u32= 1;
            if cap[2].len() > 0 {
                day = match (&cap[2]).parse::<u32>() {
                    Ok(value) => value,
                    _ => return None
                }
            }

            let month = regex_to_month((&cap[1]).to_string()).number_from_month();

            let d = date.clone();
            let res = d.with_day(1)
                .and_then(|d| d.with_month(month))
                .and_then(|d| d.with_day(day));

            let Some(res) = res else { return None; };

            if (res - date).num_days() < 0 {
                return res.with_year(res.year() + 1);
            } else { return Some(res); };
        }
    }

    // 0 (today), -1 (yesterday), 1(tomorrow), 2, -2, ...
    let Ok(re) = Regex::new(r"^(-{0,1})(\d+)(d|day|days)*$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(n) = (&cap[2]).parse::<u64>() else { return None; };

            let is_negative = cap[1].len() > 0;
            let date = match is_negative {
                true => date.checked_sub_days(Days::new(n)),
                false => date.checked_add_days(Days::new(n))
            };
            return date
        }
    }

    // sow, som, soy (start of current week, month, year)
    let Ok(re) = Regex::new(r"^(so)(w|m|y)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let weekday_diff = date.weekday().number_from_monday() - 1;
            let date = match &cap[2] {
                "w" => date.checked_sub_days(Days::new(weekday_diff as u64)),
                "m" => date.with_day(1),
                "y" => {
                    date.with_day(1).and_then(|d| d.with_month(1))
                }
                _ => Some(date)
            };

            return date;
        }
    }

    // eow, eom, eoy (start of current week, month, year)
    let Ok(re) = Regex::new(r"^(eo)(w|m|y)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let weekday_diff = if date.weekday().number_from_sunday() != 1 {
                7 - (date.weekday().number_from_sunday() - 1)
            } else { 0 }; // we are already on sunday

            let date = match &cap[2] {
                "w" => date.checked_add_days(Days::new(weekday_diff as u64)),
                "m" => {
                    date.with_day(1)
                        .and_then(|d| d.checked_add_months(Months::new(1)))
                        .and_then(|d| d.checked_sub_days(Days::new(1)))
                }
                "y" => {
                    date.with_day(1)
                        .and_then(|d| d.with_month(1))
                        .and_then(|d| d.checked_add_months(Months::new(12)))
                        .and_then(|d| d.checked_sub_days(Days::new(1)))
                }
                _ => Some(date)
            };

            return date;
        }
    }

    // 1w, -2w, 2m, -3y, ...
    let Ok(re) = Regex::new(r"^(-{0,1})(\d+)(w|m|y)$") else { return None;};
    if re.is_match(&date_string) {
        if let Some(cap) = re.captures(&date_string) {
            let Ok(n) = (&cap[2]).parse::<u64>() else { return None; };
            let is_negative = cap[1].len() > 0;

            let date = match (is_negative, &cap[3]) {
                (false, "w") => date.checked_add_days(Days::new(7*n)),
                (false, "m") => date.checked_add_months(Months::new(n as u32)),
                (false, "y") => date.checked_add_months(Months::new(12*n as u32)),
                (true, "w") => date.checked_sub_days(Days::new(7*n)),
                (true, "m") => date.checked_sub_months(Months::new(n as u32)),
                (true, "y") => date.checked_sub_months(Months::new(12*n as u32)),
                _ => Some(date)
            };

            return date;
        }
    }

    // now
    let Ok(re) = Regex::new(r"^(now|today|heute|jetzt|current|cur)$") else { return None;};
    if re.is_match(&date_string) {
        return Some(Local::now().naive_local());
    }
    return None;
}

fn regex_to_weekday(reg: String) -> chrono::Weekday
{
    match reg.as_str()
    {
        "mo" => chrono::Weekday::Mon,
        "mon" => chrono::Weekday::Mon,
        "di" => chrono::Weekday::Tue,
        "tu" => chrono::Weekday::Tue,
        "tue" => chrono::Weekday::Tue,
        "mi" => chrono::Weekday::Wed,
        "we" => chrono::Weekday::Wed,
        "wed" => chrono::Weekday::Wed,
        "do" => chrono::Weekday::Thu,
        "thu" => chrono::Weekday::Thu,
        "th" => chrono::Weekday::Thu,
        "fr" => chrono::Weekday::Fri,
        "fri" => chrono::Weekday::Fri,
        "sa" => chrono::Weekday::Sat,
        "sat" => chrono::Weekday::Sat,
        "su" => chrono::Weekday::Sun,
        "so" => chrono::Weekday::Sun,
        "sun" => chrono::Weekday::Sun,
        _ => chrono::Weekday::Mon
    }
}

fn regex_to_month(reg: String) -> chrono::Month
{
    match reg.as_str()
    {
        "jan" => chrono::Month::January,
        "ja" => chrono::Month::January,
        "feb" => chrono::Month::February,
        "fe" => chrono::Month::February,
        "ma" => chrono::Month::March,
        "mä" => chrono::Month::March,
        "apr" => chrono::Month::April,
        "ap" => chrono::Month::April,
        "may" => chrono::Month::May,
        "mai" => chrono::Month::May,
        "jun" => chrono::Month::June,
        "june" => chrono::Month::June,
        "juni" => chrono::Month::June,
        "jul" => chrono::Month::July,
        "july" => chrono::Month::July,
        "juli" => chrono::Month::July,
        "aug" => chrono::Month::August,
        "au" => chrono::Month::August,
        "sep" => chrono::Month::September,
        "se" => chrono::Month::September,
        "okt" => chrono::Month::October,
        "oct" => chrono::Month::October,
        "oc" => chrono::Month::October,
        "ok" => chrono::Month::October,
        "nov" => chrono::Month::November,
        "no" => chrono::Month::November,
        "dec" => chrono::Month::December,
        "dez" => chrono::Month::December,
        "de" => chrono::Month::December,
        _ => chrono::Month::January
    }
}


#[cfg(test)]
mod test {
    use chrono::TimeZone;
    use super::*;

    #[test]
    fn test() {
        let date_format = "%d-%m-%Y %H:%M:%S %A";
        let base = Local.with_ymd_and_hms(2025, 4, 2, 13, 22, 34).unwrap(); // Wed

        assert_eq!(base.format(date_format).to_string(), "02-04-2025 13:22:34 Wednesday");

        let date = parse_date("15:45", base).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 15:45:00 Wednesday");

        let date = parse_date("02:04", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 02:04:00 Wednesday");

        let date = parse_date("23:56", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 23:56:00 Wednesday");

        let date = parse_date("2:1", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 02:01:00 Wednesday");

        let date = parse_date("1645", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 16:45:00 Wednesday");

        let date = parse_date("0004", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 00:04:00 Wednesday");

        let date = parse_date("1835", date).unwrap();
        assert_eq!(date.format(date_format).to_string(), "02-04-2025 18:35:00 Wednesday");

        let tmp = parse_date("fri", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "04-04-2025 18:35:00 Friday");

        let tmp = parse_date("fri1", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "11-04-2025 18:35:00 Friday");

        let tmp = parse_date("fri-", date.clone()).unwrap(); 
        assert_eq!(tmp.format(date_format).to_string(), "28-03-2025 18:35:00 Friday"); 

        let tmp = parse_date("fri-1", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "21-03-2025 18:35:00 Friday");

        let tmp = parse_date("fri-2", date.clone()).unwrap(); 
        assert_eq!(tmp.format(date_format).to_string(), "14-03-2025 18:35:00 Friday");

        let tmp = parse_date("12dec", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "12-12-2025 18:35:00 Friday");

        let tmp = parse_date("5jul", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "05-07-2025 18:35:00 Saturday");

        let tmp = parse_date("20ma", date.clone()).unwrap(); // ma = march
        assert_eq!(tmp.format(date_format).to_string(), "20-03-2026 18:35:00 Friday");

        let tmp = parse_date("1apr", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "01-04-2026 18:35:00 Wednesday");

        let tmp = parse_date("2ma", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "02-03-2026 18:35:00 Monday");

        let tmp = parse_date("17ma", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-03-2026 18:35:00 Tuesday");

        let tmp = parse_date("18ma", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "18-03-2026 18:35:00 Wednesday");

        let tmp = parse_date("19may", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "19-05-2025 18:35:00 Monday");

        let tmp = parse_date("1.6", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "01-06-2025 18:35:00 Sunday");

        let tmp = parse_date("24.6.2023", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "24-06-2023 18:35:00 Saturday");

        let tmp = parse_date("17.3", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-03-2026 18:35:00 Tuesday");

        let tmp = parse_date("18.3", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "18-03-2026 18:35:00 Wednesday");

        let tmp = parse_date("19.3", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "19-03-2026 18:35:00 Thursday");

        let tmp = parse_date("17.3.2024", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-03-2024 18:35:00 Sunday");

        let tmp = parse_date("17.3.2025", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-03-2025 18:35:00 Monday");

        let tmp = parse_date("17.3.2026", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-03-2026 18:35:00 Tuesday");

        let tmp = parse_date("2.", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "02-04-2025 18:35:00 Wednesday");

        let tmp = parse_date("17.", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "17-04-2025 18:35:00 Thursday");

        let tmp = parse_date("18.", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "18-04-2025 18:35:00 Friday");

        let tmp = parse_date("19.", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "19-04-2025 18:35:00 Saturday");

        let tmp = parse_date("1.w", date.clone()).unwrap(); 
        assert_eq!(tmp.format(date_format).to_string(), "30-12-2024 18:35:00 Monday");

        let tmp = parse_date("2.w", date.clone()).unwrap();
        assert_eq!(tmp.format(date_format).to_string(), "06-01-2025 18:35:00 Monday");
    }
}
