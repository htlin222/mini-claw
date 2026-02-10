use crate::browser::BrowserSession;

fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

pub async fn goto(session: &mut BrowserSession, url: &str) -> anyhow::Result<serde_json::Value> {
    let url = normalize_url(url);
    session.page.goto(&url).await?;

    Ok(serde_json::json!({
        "url": session.url().await,
        "title": session.title().await,
    }))
}

pub async fn back(session: &mut BrowserSession) -> anyhow::Result<serde_json::Value> {
    session
        .page
        .evaluate("window.history.back()")
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    Ok(serde_json::json!({
        "url": session.url().await,
        "title": session.title().await,
    }))
}

pub async fn forward(session: &mut BrowserSession) -> anyhow::Result<serde_json::Value> {
    session
        .page
        .evaluate("window.history.forward()")
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    Ok(serde_json::json!({
        "url": session.url().await,
        "title": session.title().await,
    }))
}

pub async fn reload(session: &mut BrowserSession) -> anyhow::Result<serde_json::Value> {
    session.page.reload().await?;

    Ok(serde_json::json!({
        "url": session.url().await,
        "title": session.title().await,
    }))
}
