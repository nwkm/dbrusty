use color_eyre::Result;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init() -> Result<()> {
    color_eyre::install()?;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    Ok(())
}
