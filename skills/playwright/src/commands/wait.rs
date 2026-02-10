use tokio::time::{Duration, timeout};

use crate::browser::BrowserSession;

pub async fn wait_selector(
    session: &mut BrowserSession,
    selector: &str,
    timeout_ms: u64,
) -> anyhow::Result<serde_json::Value> {
    let sel = selector.to_string();
    let page = &session.page;

    let result = timeout(Duration::from_millis(timeout_ms), async {
        loop {
            let found = page
                .evaluate(format!(
                    "!!document.querySelector('{}')",
                    sel.replace('\'', "\\'")
                ))
                .await
                .ok()
                .and_then(|v| v.into_value::<bool>().ok())
                .unwrap_or(false);

            if found {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(serde_json::json!({
            "url": session.url().await,
        })),
        _ => Err(anyhow::anyhow!(
            "Timeout waiting for selector: {selector}"
        )),
    }
}

pub async fn wait_text(
    session: &mut BrowserSession,
    text: &str,
    timeout_ms: u64,
) -> anyhow::Result<serde_json::Value> {
    let search_text = text.to_string();
    let page = &session.page;

    let result = timeout(Duration::from_millis(timeout_ms), async {
        loop {
            let found = page
                .evaluate(format!(
                    "document.body.innerText.includes('{}')",
                    search_text.replace('\'', "\\'")
                ))
                .await
                .ok()
                .and_then(|v| v.into_value::<bool>().ok())
                .unwrap_or(false);

            if found {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(serde_json::json!({
            "url": session.url().await,
        })),
        _ => Err(anyhow::anyhow!("Timeout waiting for text: {text}")),
    }
}

pub async fn wait_navigation(
    session: &mut BrowserSession,
    timeout_ms: u64,
) -> anyhow::Result<serde_json::Value> {
    // Wait for the page to reach a loaded state
    let page = &session.page;

    let result = timeout(Duration::from_millis(timeout_ms), async {
        loop {
            let state = page
                .evaluate("document.readyState")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_default();

            if state == "complete" || state == "interactive" {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(serde_json::json!({
            "url": session.url().await,
        })),
        _ => Err(anyhow::anyhow!("Timeout waiting for navigation")),
    }
}

pub async fn wait(
    session: &mut BrowserSession,
    ms: u64,
) -> anyhow::Result<serde_json::Value> {
    tokio::time::sleep(Duration::from_millis(ms)).await;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}
