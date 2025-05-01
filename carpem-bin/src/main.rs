use clap::Parser;
use cli::Cli;
use crate::app::App;
use anyhow::Result;
mod action;
mod app;
mod cli;
mod config;
mod logging;
mod tui;
mod components; 

mod utils {
    pub mod multi_list;
    pub mod user_interaction;
    pub mod calendar_widget;
    pub mod table_widget;
    pub mod ui_navigation;
}

#[tokio::main]
async fn main() -> Result<()> {
    crate::logging::init()?;
    let args = Cli::parse();

    let mut app = App::new(args.tick_rate, args.frame_rate)?;
    app.run().await?;
    Ok(())
}
