use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;

pub struct BrowserSession {
    pub page: Page,
}

impl BrowserSession {
    pub async fn url(&self) -> String {
        self.page.url().await.ok().flatten().unwrap_or_default().to_string()
    }

    pub async fn title(&self) -> String {
        self.page
            .evaluate("document.title")
            .await
            .ok()
            .and_then(|v| v.into_value::<String>().ok())
            .unwrap_or_default()
    }
}

pub async fn get_browser() -> anyhow::Result<BrowserSession> {
    // For CLI usage, create a fresh browser each time
    let config = BrowserConfig::builder()
        .no_sandbox()
        .window_size(1280, 720)
        .arg("--disable-setuid-sandbox")
        .arg("--headless=new")
        .build()
        .map_err(|e| anyhow::anyhow!("Browser config error: {e}"))?;

    let (browser, mut handler) = Browser::launch(config).await?;

    // Spawn handler task
    tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            // Process browser events
            let _ = event;
        }
    });

    let page = browser.new_page("about:blank").await?;

    // Store for later cleanup
    // Note: for the CLI, each invocation is a new process, so we don't need
    // to worry about browser reuse across commands
    Ok(BrowserSession { page })
}

pub async fn close_browser() {
    // Browser will be dropped when the process exits
    // For explicit cleanup, we could store the browser handle
}

pub async fn get_status() -> serde_json::Value {
    serde_json::json!({
        "connected": false,
        "note": "Each CLI invocation creates a fresh browser"
    })
}
