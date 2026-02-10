use chromiumoxide::page::ScreenshotParams;

use crate::browser::BrowserSession;

pub async fn screenshot(
    session: &mut BrowserSession,
    output: Option<&str>,
    full_page: bool,
) -> anyhow::Result<serde_json::Value> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let path = output
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("/tmp/pw-screenshot-{timestamp}.png"));

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let params = ScreenshotParams::builder()
        .full_page(full_page)
        .build();

    let bytes = session.page.screenshot(params).await?;
    tokio::fs::write(&path, &bytes).await?;

    Ok(serde_json::json!({
        "path": path,
        "url": session.url().await,
    }))
}
