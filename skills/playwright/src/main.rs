mod browser;
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pw", about = "Browser automation CLI using Chrome DevTools Protocol")]
struct Cli {
    #[command(subcommand)]
    command: PwCommand,
}

#[derive(Subcommand)]
enum PwCommand {
    /// Navigate to a URL
    #[command(alias = "goto")]
    Navigate {
        /// URL to navigate to
        url: String,
    },
    /// Go back in browser history
    Back,
    /// Go forward in browser history
    Forward,
    /// Reload the current page
    Reload,
    /// Take a screenshot
    Screenshot {
        /// Output path (default: /tmp/pw-screenshot-<timestamp>.png)
        #[arg(short, long)]
        output: Option<String>,
        /// Capture full page
        #[arg(short, long)]
        full_page: bool,
    },
    /// Click an element by CSS selector
    Click {
        /// CSS selector
        selector: String,
    },
    /// Type text into an element (appends)
    Type {
        /// CSS selector
        selector: String,
        /// Text to type
        text: String,
    },
    /// Fill an input element (replaces)
    Fill {
        /// CSS selector
        selector: String,
        /// Value to fill
        value: String,
    },
    /// Select a dropdown option
    Select {
        /// CSS selector
        selector: String,
        /// Option value
        value: String,
    },
    /// Hover over an element
    Hover {
        /// CSS selector
        selector: String,
    },
    /// Focus an element
    Focus {
        /// CSS selector
        selector: String,
    },
    /// Press a keyboard key
    Press {
        /// Key to press (e.g., "Enter", "Tab", "Escape")
        key: String,
    },
    /// Get page content (text or HTML)
    Content {
        /// Output format: text or html
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Get text content of an element
    Text {
        /// CSS selector
        selector: String,
    },
    /// Get accessibility tree snapshot
    Snapshot,
    /// Wait for a selector to appear
    WaitSelector {
        /// CSS selector to wait for
        selector: String,
        /// Timeout in milliseconds
        #[arg(short, long, default_value = "30000")]
        timeout: u64,
    },
    /// Wait for text to appear on page
    WaitText {
        /// Text to wait for
        text: String,
        /// Timeout in milliseconds
        #[arg(short, long, default_value = "30000")]
        timeout: u64,
    },
    /// Wait for navigation to complete
    WaitNavigation {
        /// Timeout in milliseconds
        #[arg(short, long, default_value = "30000")]
        timeout: u64,
    },
    /// Wait for a specified time
    Wait {
        /// Milliseconds to wait
        ms: u64,
    },
    /// Navigate to URL and get content or screenshot
    Fetch {
        /// URL to fetch
        url: String,
        /// Output path for screenshot
        #[arg(short, long)]
        output: Option<String>,
        /// Capture full page screenshot
        #[arg(short, long)]
        full_page: bool,
        /// Content format: text or html
        #[arg(long, default_value = "text")]
        format: String,
        /// Take screenshot instead of getting content
        #[arg(long)]
        screenshot: bool,
    },
    /// Check browser connection status
    Status,
    /// Close the browser
    Close,
}

fn json_success(fields: serde_json::Value) -> String {
    let mut obj = fields.as_object().cloned().unwrap_or_default();
    obj.insert("success".into(), serde_json::Value::Bool(true));
    obj.insert(
        "timestamp".into(),
        serde_json::Value::String(timestamp()),
    );
    serde_json::to_string(&obj).unwrap()
}

fn json_error(error: &str) -> String {
    serde_json::json!({
        "success": false,
        "error": error,
        "timestamp": timestamp(),
    })
    .to_string()
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    // Simple ISO-like timestamp
    format!("{}Z", now.as_secs())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let output = match run_command(cli.command).await {
        Ok(json) => json,
        Err(e) => json_error(&e.to_string()),
    };

    println!("{output}");
}

async fn run_command(cmd: PwCommand) -> anyhow::Result<String> {
    match cmd {
        PwCommand::Navigate { url } => {
            let mut session = browser::get_browser().await?;
            let result = commands::navigate::goto(&mut session, &url).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Back => {
            let mut session = browser::get_browser().await?;
            let result = commands::navigate::back(&mut session).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Forward => {
            let mut session = browser::get_browser().await?;
            let result = commands::navigate::forward(&mut session).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Reload => {
            let mut session = browser::get_browser().await?;
            let result = commands::navigate::reload(&mut session).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Screenshot { output, full_page } => {
            let mut session = browser::get_browser().await?;
            let result = commands::screenshot::screenshot(&mut session, output.as_deref(), full_page).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Click { selector } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::click(&mut session, &selector).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Type { selector, text } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::type_text(&mut session, &selector, &text).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Fill { selector, value } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::fill(&mut session, &selector, &value).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Select { selector, value } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::select(&mut session, &selector, &value).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Hover { selector } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::hover(&mut session, &selector).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Focus { selector } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::focus(&mut session, &selector).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Press { key } => {
            let mut session = browser::get_browser().await?;
            let result = commands::interact::press(&mut session, &key).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Content { format } => {
            let mut session = browser::get_browser().await?;
            let result = commands::content::content(&mut session, &format).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Text { selector } => {
            let mut session = browser::get_browser().await?;
            let result = commands::content::text(&mut session, &selector).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Snapshot => {
            let mut session = browser::get_browser().await?;
            let result = commands::content::snapshot(&mut session).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::WaitSelector { selector, timeout } => {
            let mut session = browser::get_browser().await?;
            let result = commands::wait::wait_selector(&mut session, &selector, timeout).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::WaitText { text, timeout } => {
            let mut session = browser::get_browser().await?;
            let result = commands::wait::wait_text(&mut session, &text, timeout).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::WaitNavigation { timeout } => {
            let mut session = browser::get_browser().await?;
            let result = commands::wait::wait_navigation(&mut session, timeout).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Wait { ms } => {
            let mut session = browser::get_browser().await?;
            let result = commands::wait::wait(&mut session, ms).await?;
            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Fetch {
            url,
            output,
            full_page,
            format,
            screenshot,
        } => {
            let mut session = browser::get_browser().await?;
            let nav_result = commands::navigate::goto(&mut session, &url).await?;

            let result = if screenshot || output.is_some() {
                let ss = commands::screenshot::screenshot(&mut session, output.as_deref(), full_page).await?;
                let mut merged = nav_result.as_object().cloned().unwrap_or_default();
                if let Some(obj) = ss.as_object() {
                    for (k, v) in obj {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                serde_json::Value::Object(merged)
            } else {
                let ct = commands::content::content(&mut session, &format).await?;
                let mut merged = nav_result.as_object().cloned().unwrap_or_default();
                if let Some(obj) = ct.as_object() {
                    for (k, v) in obj {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                serde_json::Value::Object(merged)
            };

            browser::close_browser().await;
            Ok(json_success(result))
        }
        PwCommand::Status => {
            let status = browser::get_status().await;
            Ok(json_success(status))
        }
        PwCommand::Close => {
            browser::close_browser().await;
            Ok(json_success(serde_json::json!({})))
        }
    }
}
