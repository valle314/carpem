use clap::Parser;

use crate::config::{get_config_dir, get_data_dir};

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
    /// tick rate, i.e. number of ticks per second
    // #[arg(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    #[arg(short, long, value_name = "FLOAT", default_value_t = 0.001)]
    pub tick_rate: f64,

    /// frame rate, i.e. number of frames per second
    // #[arg(short, long, value_name = "FLOAT", default_value_t = 60.0)]
    #[arg(short, long, value_name = "FLOAT", default_value_t = 0.001)]
    pub frame_rate: f64,
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION")
);

pub fn version() -> String {
    let author = clap::crate_authors!();

    // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}
Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
    )
}
