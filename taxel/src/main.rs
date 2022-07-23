use taxel::eric;

fn main() -> Result<(), anyhow::Error> {
    let config = eric::configure()?;
    eric::init(config.plugin_path, config.log_path)?;
    eric::close()?;

    Ok(())
}
