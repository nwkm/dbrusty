fn main() -> color_eyre::Result<()> {
    dbrusty::logging::init()?;
    let config = dbrusty::config::AppConfig::load()?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let mut app = dbrusty::app::App::with_config(config);
        app.run().await
    })
}
