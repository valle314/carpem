use std::collections::HashMap;
use std::io::BufReader;
use std::rc::Rc;
use anyhow::Context;
use anyhow::Result;
use std::io::Lines;
use std::io::BufRead;
use std::{ fs, io::Write, process::Stdio, usize };
use std::fs::File;
use chrono::Duration;
use chrono::Local;
use chrono::NaiveDateTime;
use chrono::TimeDelta;
use chrono::Utc;
use crossterm::event::KeyCode;
use carpem_core::utils::status::Status;
use carpem_core::utils::tag::Tag;
use carpem_core::utils::task::Task;
use carpem_core::TaskManager;
use carpem_core::utils;
use date_parser;

use ratatui::{
    prelude::*, 
    widgets::{
        Paragraph,
        Block,
    },
    style::Color,
};

use tui_input::InputRequest;

use crate::tui::Tui;
use crate::tui;

enum TomlMode {
    Title,
    Description,
    Severity
}


#[derive(Debug)]
pub struct FZFResult {
    pub query: Option<String>,
    pub selected: Vec<String>
}

impl FZFResult {
    pub fn new_empty() -> Self {
        Self { query: None, selected: vec![]}
    }
}


pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(layout::Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(layout::Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn fzf_select(tui: &mut Tui, options: &Vec<String>) -> Result<FZFResult> {
    tui.exit()?;
    tui.terminal.clear()?;

    let input = options.join("\n");
    let mut child = std::process::Command::new("fzf")
        .arg("--print-query")
        .arg("--multi")
        .arg("--bind")
        .arg("tab:toggle+change-query()") // This must be one arg
        .stdin(Stdio::piped()) 
        .stdout(Stdio::piped()) 
        .spawn()?;

    let stdin = child.stdin.as_mut()
        .with_context(|| "cannot write to sdtin for fzf selection!")?;
    stdin.write_all(input.as_bytes())?;

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            tui.enter()?;
            tui.terminal.clear()?;
            return Err(anyhow::Error::new(e)).with_context(|| "failed to get output from fzf selection!");
        }
    };

    tui.enter()?;
    tui.terminal.clear()?;

    let output_vec = String::from_utf8_lossy(&output.stdout).trim_end().split("\n").map(|s| s.to_string()).collect::<Vec<String>>();

    match output_vec.len() {
        1 => { // return the query, no match found
            let trimmed_user_input = output_vec[0].trim();
            if !trimmed_user_input.is_empty() {
                return Ok(FZFResult {
                    query: Some(trimmed_user_input.to_string()),
                    selected: vec![]
                });
            } else { return Ok(FZFResult::new_empty()); }
        }, 
        n if n >= 2 => { // match found
            let trimmed_user_input = output_vec[0].trim();
            // let trimmed_match = output_vec[1].trim();
            let mut selected_items: Vec<String> = vec![];
            for selection in output_vec[1..].iter() {
                selected_items.push(selection.trim().to_string());
            }
            return Ok(FZFResult {
                query: Some(trimmed_user_input.to_string()),
                selected: selected_items
            });
        } 
        _ => return Ok(FZFResult::new_empty())
    }
}

pub fn fzf_select_indices(tui: &mut Tui, options: &Vec<String>) -> Result<Vec<usize>> {
    tui.exit()?;
    tui.terminal.clear()?;

    let input = options.join("\0");
    let mut child = std::process::Command::new("fzf")
        .arg("--multi")
        .arg("--bind")
        .arg("tab:toggle+change-query()") 
        .arg("--print0") 

        .arg("--read0")
        .arg("--with-nth")
        .arg("2..")
        .stdin(Stdio::piped()) 
        .stdout(Stdio::piped()) 
        .spawn()?;

    let stdin = child.stdin.as_mut()
        .with_context(|| "cannot write to sdtin for fzf selection!")?;
    stdin.write_all(input.as_bytes())?;

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            tui.enter()?;
            tui.terminal.clear()?;
            return Err(anyhow::Error::new(e)).with_context(|| "failed to get output from fzf selection!");
        }
    };

    tui.enter()?;
    tui.terminal.clear()?;

    let mut selected_indices = vec![];
    let s = String::from_utf8_lossy(&output.stdout);
    for substring in s.split("\0").into_iter() {
        if !s.is_empty() {
            let Some(index) = substring.split(' ').next() else { continue; };
            let Ok(index) = index.parse() else { continue; };
            selected_indices.push(index);
        }
    }
    Ok(selected_indices)
}

pub fn task_from_reader(reader: Lines<BufReader<File>>) -> Task {
    let mut title_string = "".to_string();
    let mut description_string = "".to_string();
    let mut severity = 0;
    let mut current_mode: Option<TomlMode> = None;
    for line in reader.map_while(Result::ok) {
        let trimmed_line = line.trim();
        let line_to_add = line.trim_end();
        match trimmed_line { 
            "[title]" => current_mode = Some(TomlMode::Title),
            "[description]" => current_mode = Some(TomlMode::Description),
            "[severity]" => current_mode = Some(TomlMode::Severity),
            _ => {
                if !line_to_add.is_empty() {
                    match current_mode {
                        Some(TomlMode::Title) => {
                            if !title_string.is_empty() { title_string.push_str("\n") };
                            title_string.push_str(line_to_add);
                        }
                        Some(TomlMode::Description) => {
                            if !description_string.is_empty() { description_string.push_str("\n") };
                            description_string.push_str(line_to_add);
                        }
                        Some(TomlMode::Severity) => {
                            let line_to_add = line_to_add.trim();
                            if !line_to_add.is_empty() {
                                match line_to_add.parse::<i64>() {
                                    Ok(num) => severity = num,
                                    Err(_) => ()
                                }
                            }
                        }
                        _ => ()
                    }
                }
            }
        }
    }

    Task::new(
        None, 
        &title_string, 
        &description_string, 
        severity, 
        vec![], 
        Status::Backlog, 
        Utc::now().naive_local(), 
        None, 
        None, 
        None, 
        None)
}

pub fn get_input_from_editor(file_contents: String, tui: &mut Tui) -> Result<Lines<BufReader<File>>> {
    // https://ratatui.rs/recipes/apps/spawn-vim/
    tui.exit()?;
    tui.terminal.clear()?;

    let mut tmp_file = tempfile::NamedTempFile::new()?;
    tmp_file.write_all(file_contents.as_bytes())?;
    let temp_path = tmp_file.path().to_str()
        .with_context(|| format!("could not convert {:?} to str!", tmp_file))?;

    let cmd = std::env::var("EDITOR").unwrap_or_else(|_| String::from("vim"));
    let cmd_args = vec![temp_path];
    std::process::Command::new(&cmd).args(&cmd_args).status()?;
    fs::read_to_string(temp_path)?;

    let file = std::fs::File::open(tmp_file)?;
    let reader = std::io::BufReader::new(file).lines();

    tui.enter()?;
    tui.terminal.clear()?;

    Ok(reader)
}

pub fn fzf_select_tags(tags_to_choose: &mut Vec<Tag>, tui: &mut Tui) -> Result<FZFResult> {
    let mut options = vec![];
    for tag in tags_to_choose.iter() {
        options.push(tag.text.clone());
    }

    fzf_select(tui, &options)
}

pub fn show_tacked_task_popup(tui: &mut Tui, taskmanager: &mut TaskManager) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    let mut rect = popup_area(tui.get_frame().area(), 60,20);

    let block = Block::bordered()
        .style(Style::new()).title("popup block");

    let tracked_tasks = taskmanager.get_tracked_tasks()?;
    let mut text_lines = vec![];
    let mut max_scroll: u16 = 0;
    for task in tracked_tasks.iter() {
        text_lines.push(Line::from(task.title.clone()));
        max_scroll += 1;
    }

    let text = Text::from(text_lines);
    let mut paragraph = Paragraph::new(text).block(block);

    let mut handled = false;
    let mut current_offset: u16 = 0;
    while !handled {
        tui.draw(|frame| { frame.render_widget(&paragraph, rect); })?;
        let event = tokio::task::block_in_place( || {
            rt.block_on(tui.next_event())
        });

        if let Some(event) = event {
            if let tui::Event::Resize(w, h) = event {
                tui.resize(Rect::new(0, 0, w, h))?;
                rect = popup_area(tui.get_frame().area(), 60,20);
            }

            if let crate::tui::Event::Key(key) = event {
                match key.code {
                    KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                        handled = true;
                    }
                    KeyCode::Char('j') => {
                        if current_offset < max_scroll {
                            current_offset += 1;
                            paragraph = paragraph.scroll((current_offset,0));
                        }
                    } 
                    KeyCode::Char('k') => {
                        if current_offset > 0 {
                            current_offset -= 1;
                            paragraph = paragraph.scroll((current_offset,0));
                        }
                    } 
                    _ => ()
                }
            }
        }
    }
    Ok(())
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(layout::Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(layout::Flex::Center).areas(area);
    area
}

fn generate_date_input_layout(tui: &mut Tui) -> Rc<[Rect]> {
    let input_area = center(
        tui.get_frame().area(),
        Constraint::Percentage(40),
        Constraint::Length(14), 
    );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7)
        ]).split(input_area);

    layout
}

pub fn get_date_input(tui: &mut Tui, start_date: NaiveDateTime) -> Result<NaiveDateTime> {
    let mut tui_input = tui_input::Input::default();
    let rt = tokio::runtime::Handle::current();

    let mut parse_hashmap = HashMap::new();
    parse_hashmap.insert(("".to_string(), start_date), start_date);

    let mut layout = generate_date_input_layout(tui);
    let date_format: &str = "%d-%m-%Y %H:%M:%S %A";
    let mut lines = vec![];
    lines.push(Line::from(format!("result: {}", start_date.format(date_format))));
    lines.push(Line::from(""));
    lines.push(Line::from("o enter => submit"));
    lines.push(Line::from("o escape => leave"));

    let mut handled = false;
    let mut result = start_date;
    let mut paragraph = Paragraph::new(lines.clone()).block(Block::bordered());

    while !handled {
        tui.draw(|frame| {
            render_tui_input(frame, layout[0], &tui_input);
            frame.render_widget(&paragraph, layout[1]);
        })?;
        let event = tokio::task::block_in_place( || {
            rt.block_on(tui.next_event())
        });

        if let Some(event) = event {
            if let tui::Event::Resize(w, h) = event {
                tui.resize(Rect::new(0, 0, w, h))?;
                layout = generate_date_input_layout(tui);
            }

            if let crate::tui::Event::Key(key) = event {
                match key.code {
                    KeyCode::Enter => {
                        handled = true;
                    }
                    KeyCode::Esc => {
                        handled = true;
                        result = start_date;
                    }
                    keycode => {
                        if let Some(req) = tui_input_handle_event(keycode) {
                            tui_input.handle(req);
                            let string_to_parse = tui_input.value().split_whitespace();
                            let mut parsed_date = start_date.clone();
                            for s in string_to_parse {
                                if let Some(hashmap_res) = parse_hashmap.get(&(s.to_string(),start_date)) {
                                    parsed_date = *hashmap_res;
                                } else {
                                    if let Some(new_date) = date_parser::parser::parse_date(s, parsed_date) {
                                        parsed_date = new_date;
                                        parse_hashmap.insert((s.to_string(),parsed_date), new_date);
                                    }
                                }
                            }
                            result = parsed_date;
                            lines[0] = Line::from(format!("result: {}", parsed_date.format(date_format)));
                        }
                    }
                }
            }
        }
        paragraph = Paragraph::new(lines.clone()).block(Block::bordered());
    }
    Ok(result)
}

pub fn render_tui_input(frame: &mut Frame, area: Rect, tui_input: &tui_input::Input) {
    // keep 2 for borders and 1 for cursor
    let width = area.width.max(3) - 3;
    let scroll = tui_input.visual_scroll(width as usize);
    // let style: Style = Color::Yellow.into();

    let input = Paragraph::new(tui_input.value())
        // .style(style)
        .scroll((0, scroll as u16))
        .block(Block::bordered().title("Input"));
    frame.render_widget(input, area);
    // Ratatui hides the cursor unless it's explicitly set. Position the  cursor past the
    // end of the input text and one line down from the border to the input line
    let x = tui_input.visual_cursor().max(scroll) - scroll + 1;
    frame.set_cursor_position((area.x + x as u16, area.y + 1))
}

pub fn tui_input_handle_event(keycode: KeyCode) -> Option<InputRequest> {
    match keycode {
        KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
        KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
        KeyCode::Left => Some(InputRequest::GoToPrevChar),
        KeyCode::Right => Some(InputRequest::GoToNextChar),
        _ => None
    }
}

fn generate_duration_input_layout(tui: &mut Tui) -> Rc<[Rect]> {
    let rect = popup_area(tui.get_frame().area(), 50, 50);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Fill(1)
        ]).split(rect);
    layout
}

fn get_duration_input(lines: &mut Vec<Line>, tui: &mut Tui) -> Result<Option<TimeDelta>> {
    let mut tui_input = tui_input::Input::default();
    let rt = tokio::runtime::Handle::current();

    // create layout
    let mut layout = generate_duration_input_layout(tui);

    let mut handled = false;
    let mut result = None;
    while !handled {
        let block = Block::bordered()
            .style(Style::new()).title("information").border_style(Style::default().fg(Color::Gray));
        let paragraph = Paragraph::new(lines.clone()).block(block);

        tui.draw(|frame| {
            render_tui_input(frame, layout[0], &tui_input);
            frame.render_widget(paragraph, layout[1]);
        })?;

        let event = tokio::task::block_in_place( || {
            rt.block_on(tui.next_event())
        });

        if let Some(event) = event {
            if let tui::Event::Resize(w, h) = event {
                tui.resize(Rect::new(0, 0, w, h))?;
                layout = generate_duration_input_layout(tui);
            }

            if let tui::Event::Key(key) = event {
                match key.code {
                    KeyCode::Enter => {
                        handled = true;
                    }
                    KeyCode::Esc => {
                        handled = true;
                        result = None;
                    }
                    keycode => {
                        if let Some(req) = tui_input_handle_event(keycode) {
                            tui_input.handle(req);
                            let string_to_parse = tui_input.value();
                            result = parse_duration(string_to_parse);
                            if let Some(time_delta) = result {
                                let total_seconds = time_delta.num_seconds();
                                let hours = total_seconds / 3600;
                                let minutes = (total_seconds % 3600) / 60;
                                lines[0] = Line::from(format!("current duration input: {:02}:{:02} (HH:MM)", hours, minutes));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(result)
}

pub fn get_event_duration_from_user(tui: &mut Tui, event: &utils::event::Event) -> Result<Option<TimeDelta>> {
    let mut lines = vec![];
    lines.push(Line::from("current duration input: (HH:MM)"));
    lines.push(Line::from("")); 
    let date_format: &str = "%d-%m-%Y %H:%M:%S %A";
    lines.push(Line::from(format!("event title: {}", event.title)));
    lines.push(Line::from(format!("event date:  {}", event.date.format(date_format))));
    let duration_text = match event.duration {
        Some(duration) => format!("event duration: {:02}:{:02}", duration.num_hours(),duration.num_hours()), 
        None => "event duration: none".to_string()
    };
    lines.push(Line::from(duration_text));
    lines.push(Line::from(""));
    lines.push(Line::from("o enter => submit"));
    lines.push(Line::from("o escape => leave"));
    get_duration_input(&mut lines, tui)
}

pub fn get_task_duration_from_user(tui: &mut Tui, task: &Task) -> Result<Option<TimeDelta>> {
    let mut lines = vec![];
    lines.push(Line::from("current duration input: (HH:MM)"));
    lines.push(Line::from("")); 
    let date_format: &str = "%d-%m-%Y %H:%M:%S %A";
    lines.push(Line::from(format!("task title: {}", task.title)));
    match task.due_date {
        Some(due_date) => lines.push(Line::from(format!("task duedate:  {}", due_date.format(date_format)))),
        None => lines.push(Line::from("duedate: none")),
    }
    match task.reminder_date {
        Some(reminder_date) => lines.push(Line::from(format!("task reminder date:  {}", reminder_date.format(date_format)))),
        None => lines.push(Line::from("reminder date: none")),
    }

    lines.push(Line::from(format!( "time taken: {}", task.time_taken .map_or("none".to_string(), |t| {
        let total_seconds = t.num_seconds();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let time = format!("{:02}:{:02}:{:02} (HH:MM:SS)", hours, minutes, seconds);
        time 
    }))));

    lines.push(Line::from(""));
    lines.push(Line::from("o enter => submit"));
    lines.push(Line::from("o escape => leave"));
    get_duration_input(&mut lines, tui)
}

fn parse_duration(input: &str) -> Option<TimeDelta> {
    let parts: Vec<&str> = input.split(':').collect();

    match parts.len() {
        1 => {
            let minutes = parse_or_zero(parts[0])?;
            Some(Duration::minutes(minutes as i64))
        }
        2 => {
            // "5:0" => 5 hours
            let hours = parse_or_zero(parts[0])?;
            let minutes = parse_or_zero(parts[1])?;
            Some(Duration::hours(hours as i64) + Duration::minutes(minutes as i64))
        }
        _ => None, // Invalid format
    }
}

fn parse_or_zero(s: &str) -> Option<u64> {
    if s.trim().is_empty() {
        Some(0)
    } else {
        s.trim().parse().ok()
    }
}


enum EventTomlMode {
    Title,
    Description
}

pub fn event_from_reader(reader: Lines<BufReader<File>>) -> utils::event::Event {
    let mut title_string = "".to_string();
    let mut description_string = "".to_string();
    let mut current_mode: Option<EventTomlMode> = None;
    for line in reader.map_while(Result::ok) {
        let trimmed_line = line.trim();
        let line_to_add = line.trim_end();
        match trimmed_line { 
            "[title]" => current_mode = Some(EventTomlMode::Title),
            "[description]" => current_mode = Some(EventTomlMode::Description),
            _ => {
                if !line_to_add.is_empty() {
                    match current_mode {
                        Some(EventTomlMode::Title) => {
                            if !title_string.is_empty() { title_string.push_str("\n") };
                            title_string.push_str(line_to_add);
                        }
                        Some(EventTomlMode::Description) => {
                            if !description_string.is_empty() { description_string.push_str("\n") };
                            description_string.push_str(line_to_add);
                        }
                        _ => ()
                    }
                }
            }
        }
    }

    utils::event::Event::new(
        None, 
        &title_string, 
        &description_string, 
        None, 
        Local::now().naive_local(),
        None, 
        None, 
        vec![])
}

pub fn get_user_number(tui: &mut Tui, event: &utils::event::Event) -> Result<Option<u32>> {
    let mut tui_input = tui_input::Input::default();
    let rt = tokio::runtime::Handle::current();

    let rect = popup_area(tui.get_frame().area(), 50,50);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Fill(1)
        ]).split(rect);


    let mut lines = vec![];
    lines.push(Line::from("current number input: "));
    lines.push(Line::from("-".repeat(layout[1].width.into())));
    let date_format: &str = "%d-%m-%Y %H:%M:%S %A";
    lines.push(Line::from(format!("event title: {}", event.title)));
    lines.push(Line::from(format!("event date:  {}", event.date.format(date_format))));
    lines.push(Line::from(""));
    lines.push(Line::from("o press escape to leave"));
    lines.push(Line::from("o press enter to submit"));

    let mut handled = false;
    let mut result = None;

    let block = Block::bordered()
        .style(Style::new()).title("event information").border_style(Style::default().fg(Color::Gray));
    let mut paragraph = Paragraph::new(lines.clone()).block(block.clone());
    while !handled {
        tui.draw(|frame| {
            render_tui_input(frame, layout[0], &tui_input);
            frame.render_widget(paragraph, layout[1]);
        })?;

        let event = tokio::task::block_in_place( || {
            rt.block_on(tui.next_event())
        });

        if let Some(event) = event {
            if let tui::Event::Key(key) = event {
                match key.code {
                    KeyCode::Enter => {
                        // result = tui_input.value().into();
                        // tui_input.reset();
                        handled = true;
                    }
                    KeyCode::Esc => {
                        handled = true;
                        result = None;
                    }
                    keycode => {
                        if let Some(req) = tui_input_handle_event(keycode) {
                            tui_input.handle(req);
                            let input = tui_input.value();
                            match input.parse::<u32>() {
                                Ok(num) => {
                                    result = Some(num);
                                    lines[0] = Line::from(format!("current number input: {}", num));
                                }
                                _ => ()
                            };
                        }
                    }
                }
            }
        }

        paragraph = Paragraph::new(lines.clone()).block(block.clone());
    }
    Ok(result)
}

fn generate_string_input_layout(tui: &mut Tui) -> Rc<[Rect]> {
    let input_area = center(
        tui.get_frame().area(),
        Constraint::Percentage(20),
        Constraint::Length(3), 
    );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
        ]).split(input_area);

    layout
}

pub fn get_string_input(tui: &mut Tui, input_text: &str) -> Result<String> {
    let mut tui_input = tui_input::Input::new(input_text.to_string());
    let rt = tokio::runtime::Handle::current();

    let mut layout = generate_string_input_layout(tui);
    let mut result = input_text.to_string();

    let mut handled = false;
    while !handled {
        tui.draw(|frame| {
            render_tui_input(frame, layout[0], &tui_input);
        })?;

        let event = tokio::task::block_in_place( || {
            rt.block_on(tui.next_event())
        });

        if let Some(event) = event {
            if let tui::Event::Resize(w, h) = event {
                tui.resize(Rect::new(0, 0, w, h))?;
                layout = generate_string_input_layout(tui);
            }

            if let tui::Event::Key(key) = event {
                match key.code {
                    KeyCode::Esc => {
                        handled = true;
                        result = "".to_string();
                    }

                    KeyCode::Enter => {
                        handled = true;
                    }

                    keycode => {
                        if let Some(req) = tui_input_handle_event(keycode) {
                            tui_input.handle(req);
                            result = tui_input.value().to_string();
                        }
                    }
                }
            }
        }
    }

    Ok(result)
}
