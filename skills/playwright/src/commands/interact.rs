use crate::browser::BrowserSession;

pub async fn click(
    session: &mut BrowserSession,
    selector: &str,
) -> anyhow::Result<serde_json::Value> {
    let element = session
        .page
        .find_element(selector)
        .await
        .map_err(|e| anyhow::anyhow!("Element not found: {e}"))?;
    element.click().await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn type_text(
    session: &mut BrowserSession,
    selector: &str,
    text: &str,
) -> anyhow::Result<serde_json::Value> {
    let element = session
        .page
        .find_element(selector)
        .await
        .map_err(|e| anyhow::anyhow!("Element not found: {e}"))?;
    element.type_str(text).await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn fill(
    session: &mut BrowserSession,
    selector: &str,
    value: &str,
) -> anyhow::Result<serde_json::Value> {
    // Clear existing value and type new one
    let js = format!(
        r#"
        const el = document.querySelector('{}');
        if (!el) throw new Error('Element not found');
        el.value = '';
        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        "#,
        selector.replace('\'', "\\'")
    );
    session.page.evaluate(js).await?;

    let element = session.page.find_element(selector).await?;
    element.type_str(value).await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn select(
    session: &mut BrowserSession,
    selector: &str,
    value: &str,
) -> anyhow::Result<serde_json::Value> {
    let js = format!(
        r#"
        const el = document.querySelector('{}');
        if (!el) throw new Error('Element not found');
        el.value = '{}';
        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
        "#,
        selector.replace('\'', "\\'"),
        value.replace('\'', "\\'")
    );
    session.page.evaluate(js).await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn hover(
    session: &mut BrowserSession,
    selector: &str,
) -> anyhow::Result<serde_json::Value> {
    let element = session
        .page
        .find_element(selector)
        .await
        .map_err(|e| anyhow::anyhow!("Element not found: {e}"))?;
    element.scroll_into_view().await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn focus(
    session: &mut BrowserSession,
    selector: &str,
) -> anyhow::Result<serde_json::Value> {
    let element = session
        .page
        .find_element(selector)
        .await
        .map_err(|e| anyhow::anyhow!("Element not found: {e}"))?;
    element.focus().await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}

pub async fn press(
    session: &mut BrowserSession,
    key: &str,
) -> anyhow::Result<serde_json::Value> {
    session
        .page
        .evaluate(format!(
            "document.dispatchEvent(new KeyboardEvent('keydown', {{ key: '{}' }}))",
            key.replace('\'', "\\'")
        ))
        .await?;

    Ok(serde_json::json!({
        "url": session.url().await,
    }))
}
