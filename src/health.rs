use reqwest::Error;

pub async fn health(port: u16) -> Result<(), Error> {
    let response = reqwest::get(&format!("http://localhost:{}/health", port)).await?;

    response.error_for_status()?;

    Ok(())
}
