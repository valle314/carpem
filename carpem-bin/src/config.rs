use std::{collections::HashMap, env, io::Write, path::PathBuf};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use derive_deref::{Deref, DerefMut};
use directories::ProjectDirs;
use lazy_static::lazy_static;
use ratatui::style::{Color, Modifier, Style};
use serde::{de::Deserializer, Deserialize};
use tracing::info;

use crate::{action::Action, components::{ComponentName, Id}};

// folder has to be correctly set at compile time only
const CONFIG: &str = include_str!("../.config/config.toml"); // config folder for development

// #[derive(Clone, Debug, Default, Deref, DerefMut, Deserialize)]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct UserData {
    pub is_time_tracked_fg: ratatui::style::Color,
    pub is_time_tracked_bg: ratatui::style::Color,
    pub status_backlog_fg: ratatui::style::Color,
    pub status_backlog_bg: ratatui::style::Color,
    pub status_pending_fg: ratatui::style::Color,
    pub status_pending_bg: ratatui::style::Color,
    pub status_in_progress_fg: ratatui::style::Color,
    pub status_in_progress_bg: ratatui::style::Color,
    pub status_testing_fg: ratatui::style::Color,
    pub status_testing_bg: ratatui::style::Color,
    pub status_done_fg: ratatui::style::Color,
    pub status_done_bg: ratatui::style::Color,

    pub table_header_color: ratatui::style::Color,
    pub table_highlighted_fg: ratatui::style::Color,
    pub table_highlighted_bg: ratatui::style::Color,
    pub text: ratatui::style::Color,
    pub border_focused: ratatui::style::Color,
    pub border_default: ratatui::style::Color,
    pub tag_highlighted_fg: ratatui::style::Color,
    pub tag_highlighted_bg: ratatui::style::Color,
    pub tag_selected_fg: ratatui::style::Color,
    pub tag_selected_bg: ratatui::style::Color,

    pub calendar_date_highlight_fg: ratatui::style::Color,
    pub calendar_date_highlight_bg: ratatui::style::Color,
    pub calendar_iso_week_fg: ratatui::style::Color,
    pub calendar_iso_week_bg: ratatui::style::Color,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub user_data: UserData,
    #[serde(default)]
    pub keybindings: KeyBindings,
}

lazy_static! {
    pub static ref PROJECT_NAME: String = env!("CARGO_CRATE_NAME").to_uppercase().to_string();
    pub static ref DATA_FOLDER: Option<PathBuf> =
        env::var(format!("{}_DATA", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref CONFIG_FOLDER: Option<PathBuf> =
        env::var(format!("{}_CONFIG", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
}

impl Config {
    pub fn new() -> Result<Self> {
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();
        let data_dir_str = data_dir.to_string_lossy().to_string();
        let config_dir_str = config_dir.to_string_lossy().to_string();
        let mut builder = config::Config::builder()
            .set_default("data_dir", data_dir_str)?
            .set_default("config_dir", config_dir_str)?;

        let config_file_name = "config.toml";
        let mut found_config = false;
        let source = config::File::from(config_dir.join(config_file_name))
            .format(config::FileFormat::Toml)
            .required(false);
        builder = builder.add_source(source);
        if config_dir.join(config_file_name).exists() {
            found_config = true
        }

        if !found_config {
            info!("the config file {}/{} does not exist! using default config instead",
                config_dir.to_string_lossy(),
                config_file_name
            );

            // create the file and write default config in it
            std::fs::create_dir_all(&config_dir)?;
            let mut file = std::fs::File::create(config_dir.join(config_file_name))?;
            file.write_all(CONFIG.as_bytes())?;

            let source = config::File::from_str(CONFIG, config::FileFormat::Toml);
            builder = builder.add_source(source);
        } 

        let cfg: Self = builder.build()?.try_deserialize()?;
        Ok(cfg)
    }
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };
    directory
}

pub fn get_config_dir() -> PathBuf {
    let directory = if let Some(s) = CONFIG_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".config")
    };
    directory
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "valle314", env!("CARGO_PKG_NAME"))
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct KeyBindings(pub HashMap<ComponentName, HashMap<Vec<KeyEvent>, Action>>);

impl KeyBindings {
    pub const MAX_SEQUENCE_LENGTH: usize = 3;
}

impl<'de> Deserialize<'de> for KeyBindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<ComponentName, HashMap<String, Action>>::deserialize(deserializer)?;

        let keybindings = parsed_map
            .into_iter()
            .map(|(name, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .map(|(key_str, cmd)| {
                        parse_key_sequence(&key_str)
                            .map_err(|e| {
                                serde::de::Error::custom(
                                    format!("deserializing failed: {}", e))
                            })
                        .map(|key_seq| (key_seq, cmd))
                    })
                .collect::<Result<_, D::Error>>()?;
                Ok((name, converted_inner_map))
            })
        .collect::<Result<_, D::Error>>()?;

        Ok(KeyBindings(keybindings))
    }
}

fn parse_key_event(raw: &str) -> Result<KeyEvent, String> {
    let raw_lower = raw.to_ascii_lowercase();
    let (remaining, modifiers) = extract_modifiers(&raw_lower);
    parse_key_code_with_modifiers(remaining, modifiers)
}

fn extract_modifiers(raw: &str) -> (&str, KeyModifiers) {
    let mut modifiers = KeyModifiers::empty();
    let mut current = raw;

    loop {
        match current {
            rest if rest.starts_with("ctrl-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                current = &rest[5..];
            }
            rest if rest.starts_with("alt-") => {
                modifiers.insert(KeyModifiers::ALT);
                current = &rest[4..];
            }
            rest if rest.starts_with("shift-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                current = &rest[6..];
            }
            _ => break, // break out of the loop if no known prefix is detected
        };
    }

    (current, modifiers)
}

fn parse_key_code_with_modifiers(
    raw: &str,
    mut modifiers: KeyModifiers,
) -> Result<KeyEvent, String> {
    let c = match raw {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        "hyphen" => KeyCode::Char('-'),
        "minus" => KeyCode::Char('-'),
        "tab" => KeyCode::Tab,
        c if c.len() == 1 => {
            let Some(mut c) = c.chars().next() else { return Err(format!("Unable to parse {raw}")); };
            if modifiers.contains(KeyModifiers::SHIFT) {
                c = c.to_ascii_uppercase();
            }
            KeyCode::Char(c)
        }
        // _ => return Err(format!("Unable to parse {raw}")),
        _ => {
            return Err(format!(
                "unknown key identifier '{raw}' in keybinding\n\
                \texpected a named key like 'enter', 'f1', 'left', or a single character"
            ))
        }

    };
    Ok(KeyEvent::new(c, modifiers))
}

#[allow(unused)]
pub fn key_event_to_string(key_event: &KeyEvent) -> String {
    let char;
    let key_code = match key_event.code {
        KeyCode::Backspace => "backspace",
        KeyCode::Enter => "enter",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        KeyCode::Home => "home",
        KeyCode::End => "end",
        KeyCode::PageUp => "pageup",
        KeyCode::PageDown => "pagedown",
        KeyCode::Tab => "tab",
        KeyCode::BackTab => "backtab",
        KeyCode::Delete => "delete",
        KeyCode::Insert => "insert",
        KeyCode::F(c) => {
            char = format!("f({c})");
            &char
        }
        KeyCode::Char(' ') => "space",
        KeyCode::Char(c) => {
            char = c.to_string();
            &char
        }
        KeyCode::Esc => "esc",
        KeyCode::Null => "",
        KeyCode::CapsLock => "",
        KeyCode::Menu => "",
        KeyCode::ScrollLock => "",
        KeyCode::Media(_) => "",
        KeyCode::NumLock => "",
        KeyCode::PrintScreen => "",
        KeyCode::Pause => "",
        KeyCode::KeypadBegin => "",
        KeyCode::Modifier(_) => "",
    };

    let mut modifiers = Vec::with_capacity(3);

    if key_event.modifiers.intersects(KeyModifiers::CONTROL) {
        modifiers.push("ctrl");
    }

    if key_event.modifiers.intersects(KeyModifiers::SHIFT) {
        modifiers.push("shift");
    }

    if key_event.modifiers.intersects(KeyModifiers::ALT) {
        modifiers.push("alt");
    }

    let mut key = modifiers.join("-");

    if !key.is_empty() {
        key.push('-');
    }
    key.push_str(key_code);

    key
}

pub fn parse_key_sequence(raw: &str) -> Result<Vec<KeyEvent>, String> {
    if raw.chars().filter(|c| *c == '>').count() != raw.chars().filter(|c| *c == '<').count() {
        return Err(format!(
                "syntax error in key sequence `{}`: unmatched angle brackets, make sure every '<' has a corresponding '>'",
                raw
        ));
    }

    let raw = if !raw.contains("><") {
        let raw = raw.strip_prefix('<').unwrap_or(raw);
        let raw = raw.strip_prefix('>').unwrap_or(raw);
        raw
    } else {
        raw
    };
    let sequences = raw
        .split("><")
        .map(|seq| {
            if let Some(s) = seq.strip_prefix('<') {
                s
            } else if let Some(s) = seq.strip_suffix('>') {
                s
            } else {
                seq
            }
        })
        .collect::<Vec<_>>();

    match sequences.len() <= KeyBindings::MAX_SEQUENCE_LENGTH {
        true => sequences.into_iter().map(parse_key_event).collect(),
        false => {
            Err(format!(
                "the maximum supported sequence lenght is {} and `{}`(len={}) is exceeding this value", 
                KeyBindings::MAX_SEQUENCE_LENGTH, 
                raw,
                sequences.len()
            ))
        }
    }
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct Styles(pub HashMap<Id, HashMap<String, Style>>);

impl<'de> Deserialize<'de> for Styles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Id, HashMap<String, String>>::deserialize(deserializer)?;

        let styles = parsed_map
            .into_iter()
            .map(|(mode, inner_map)| {
                let converted_inner_map = inner_map
                    .into_iter()
                    .map(|(str, style)| (str, parse_style(&style)))
                    .collect();
                (mode, converted_inner_map)
            })
            .collect();

        Ok(Styles(styles))
    }
}

pub fn parse_style(line: &str) -> Style {
    let (foreground, background) =
        line.split_at(line.to_lowercase().find("on ").unwrap_or(line.len()));
    let foreground = process_color_string(foreground);
    let background = process_color_string(&background.replace("on ", ""));

    let mut style = Style::default();
    if let Some(fg) = parse_color(&foreground.0) {
        style = style.fg(fg);
    }
    if let Some(bg) = parse_color(&background.0) {
        style = style.bg(bg);
    }
    style = style.add_modifier(foreground.1 | background.1);
    style
}

fn process_color_string(color_str: &str) -> (String, Modifier) {
    let color = color_str
        .replace("grey", "gray")
        .replace("bright ", "")
        .replace("bold ", "")
        .replace("underline ", "")
        .replace("inverse ", "");

    let mut modifiers = Modifier::empty();
    if color_str.contains("underline") {
        modifiers |= Modifier::UNDERLINED;
    }
    if color_str.contains("bold") {
        modifiers |= Modifier::BOLD;
    }
    if color_str.contains("inverse") {
        modifiers |= Modifier::REVERSED;
    }

    (color, modifiers)
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim_start();
    let s = s.trim_end();
    if s.contains("bright color") {
        let s = s.trim_start_matches("bright ");
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c.wrapping_shl(8)))
    } else if s.contains("color") {
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c))
    } else if s.contains("gray") {
        let c = 232
            + s.trim_start_matches("gray")
                .parse::<u8>()
                .unwrap_or_default();
        Some(Color::Indexed(c))
    } else if s.contains("rgb") {
        let red = (s.as_bytes()[3] as char).to_digit(10).unwrap_or_default() as u8;
        let green = (s.as_bytes()[4] as char).to_digit(10).unwrap_or_default() as u8;
        let blue = (s.as_bytes()[5] as char).to_digit(10).unwrap_or_default() as u8;
        let c = 16 + red * 36 + green * 6 + blue;
        Some(Color::Indexed(c))
    } else if s == "bold black" {
        Some(Color::Indexed(8))
    } else if s == "bold red" {
        Some(Color::Indexed(9))
    } else if s == "bold green" {
        Some(Color::Indexed(10))
    } else if s == "bold yellow" {
        Some(Color::Indexed(11))
    } else if s == "bold blue" {
        Some(Color::Indexed(12))
    } else if s == "bold magenta" {
        Some(Color::Indexed(13))
    } else if s == "bold cyan" {
        Some(Color::Indexed(14))
    } else if s == "bold white" {
        Some(Color::Indexed(15))
    } else if s == "black" {
        Some(Color::Indexed(0))
    } else if s == "red" {
        Some(Color::Indexed(1))
    } else if s == "green" {
        Some(Color::Indexed(2))
    } else if s == "yellow" {
        Some(Color::Indexed(3))
    } else if s == "blue" {
        Some(Color::Indexed(4))
    } else if s == "magenta" {
        Some(Color::Indexed(5))
    } else if s == "cyan" {
        Some(Color::Indexed(6))
    } else if s == "white" {
        Some(Color::Indexed(7))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_style_default() {
        let style = parse_style("");
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_parse_style_foreground() {
        let style = parse_style("red");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
    }

    #[test]
    fn test_parse_style_background() {
        let style = parse_style("on blue");
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_parse_style_modifiers() {
        let style = parse_style("underline red on blue");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_process_color_string() {
        let (color, modifiers) = process_color_string("underline bold inverse gray");
        assert_eq!(color, "gray");
        assert!(modifiers.contains(Modifier::UNDERLINED));
        assert!(modifiers.contains(Modifier::BOLD));
        assert!(modifiers.contains(Modifier::REVERSED));
    }

    #[test]
    fn test_parse_color_rgb() {
        let color = parse_color("rgb123");
        let expected = 16 + 36 + 2 * 6 + 3;
        assert_eq!(color, Some(Color::Indexed(expected)));
    }

    #[test]
    fn test_parse_color_unknown() {
        let color = parse_color("unknown");
        assert_eq!(color, None);
    }

    #[test]
    fn test_simple_keys() {
        let Ok(key) = parse_key_event("a") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())
        );

        let Ok(key) = parse_key_event("enter") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
        );

        let Ok(key) = parse_key_event("esc") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
        );
    }

    #[test]
    fn test_with_modifiers() {
        let Ok(key) = parse_key_event("ctrl-a") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        let Ok(key) = parse_key_event("alt-enter") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );

        let Ok(key) = parse_key_event("shift-esc") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_multiple_modifiers() {
        let Ok(key) = parse_key_event("ctrl-alt-a") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );

        let Ok(key) = parse_key_event("ctrl-shift-enter") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_reverse_multiple_modifiers() {
        assert_eq!(
            key_event_to_string(&KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            "ctrl-alt-a".to_string()
        );
    }

    #[test]
    fn test_invalid_keys() {
        assert!(parse_key_event("invalid-key").is_err());
        assert!(parse_key_event("ctrl-invalid-key").is_err());
    }

    #[test]
    fn test_case_insensitivity() {
        let Ok(key) = parse_key_event("CTRL-a") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        let Ok(key) = parse_key_event("AlT-eNtEr") else { return; };
        assert_eq!(
            key,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );
    }
}
