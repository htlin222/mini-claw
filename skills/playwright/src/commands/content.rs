use crate::browser::BrowserSession;

pub async fn content(
    session: &mut BrowserSession,
    format: &str,
) -> anyhow::Result<serde_json::Value> {
    let content = if format == "html" {
        session.page.content().await?
    } else {
        session
            .page
            .evaluate("document.body.innerText")
            .await?
            .into_value::<String>()
            .unwrap_or_default()
    };

    Ok(serde_json::json!({
        "content": content,
        "url": session.url().await,
    }))
}

pub async fn text(
    session: &mut BrowserSession,
    selector: &str,
) -> anyhow::Result<serde_json::Value> {
    let element = session
        .page
        .find_element(selector)
        .await
        .map_err(|e| anyhow::anyhow!("Element not found: {e}"))?;

    let text = element
        .inner_text()
        .await?
        .unwrap_or_default();

    Ok(serde_json::json!({
        "text": text.trim(),
        "url": session.url().await,
    }))
}

pub async fn snapshot(
    session: &mut BrowserSession,
) -> anyhow::Result<serde_json::Value> {
    // Use CDP Accessibility.getFullAXTree for accessibility snapshot
    let snapshot = session
        .page
        .evaluate(
            r#"
            (function() {
                function getAccessibilityTree(node, depth) {
                    if (depth > 10) return '';
                    let result = '';
                    const indent = '  '.repeat(depth);
                    const role = node.getAttribute && node.getAttribute('role') || node.nodeName.toLowerCase();
                    const text = node.textContent ? node.textContent.trim().substring(0, 100) : '';
                    const ariaLabel = node.getAttribute && node.getAttribute('aria-label') || '';

                    if (role !== '#text' && role !== 'script' && role !== 'style') {
                        let line = `${indent}${role}`;
                        if (ariaLabel) line += ` "${ariaLabel}"`;
                        else if (text && !node.children.length) line += ` "${text}"`;
                        result += line + '\n';
                    }

                    for (const child of (node.children || [])) {
                        result += getAccessibilityTree(child, depth + 1);
                    }
                    return result;
                }
                return getAccessibilityTree(document.body, 0);
            })()
            "#,
        )
        .await?
        .into_value::<String>()
        .unwrap_or_default();

    Ok(serde_json::json!({
        "snapshot": snapshot,
        "url": session.url().await,
    }))
}
