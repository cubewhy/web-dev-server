pub const DEFAULT_PORT: u16 = 3000;

#[derive(Debug, Clone, clap::Parser)]
pub struct DevServerConfig {
    #[clap(
        long,
        default_value_t = DEFAULT_PORT,
        help = "Port to run the development server on"
    )]
    pub port: u16,
    #[clap(
        default_value = "./",
        help = "Base directory for the development server"
    )]
    pub base_dir: String,
    #[clap(
        long,
        default_value_t = false,
        help = "Enable diff mode to update HTML/CSS without full page reloads"
    )]
    pub diff_mode: bool,
    #[clap(
        long = "no-open-browser",
        default_value_t = false,
        action = clap::ArgAction::SetTrue,
        help = "Disable automatically opening the default browser"
    )]
    pub no_open_browser: bool,
}
