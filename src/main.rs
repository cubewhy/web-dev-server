use clap::Parser;
use web_dev_server::{cli, startup::Application};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = web_dev_server::config::DevServerConfig::parse();
    let app = Application::build(&config).await?;
    cli::print_startup_summary(&config, &app);
    if !config.no_open_browser {
        cli::launch_browser(app.primary_url());
    }
    app.run_until_stopped().await?;

    Ok(())
}
