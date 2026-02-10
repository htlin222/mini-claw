use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{ChatAction, InputFile};
use tokio::time::{Duration, Instant};

use super::util::split_message;
use super::AppState;
use crate::file_detector::{detect_files, snapshot_workspace};
use crate::markdown::{markdown_to_html, strip_markdown};
use crate::pi_rpc::PiEvent;
use crate::pi_runner::{
    extract_images_from_session, get_session_line_count, run_pi_with_streaming, ActivityType,
    ActivityUpdate, RunPiOptions,
};

pub async fn handle_text(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    // Skip commands
    if text.starts_with('/') {
        return Ok(());
    }

    // Access control
    if !state.check_access(&msg) {
        bot.send_message(msg.chat.id, "Sorry, you are not authorized to use this bot.")
            .await?;
        return Ok(());
    }

    let chat_id = msg.chat.id.0;

    // Rate limiting
    {
        let mut limiter = state.rate_limiter.lock().await;
        let result = limiter.check(chat_id, state.config.rate_limit_cooldown_ms);
        if !result.allowed {
            let secs = result.retry_after_ms.unwrap_or(0).div_ceil(1000);
            bot.send_message(
                msg.chat.id,
                format!("\u{23f3} Please wait {secs}s before sending another message."),
            )
            .await?;
            return Ok(());
        }
    }

    // Check if live mode is active
    let live_active = state.live_sessions.lock().await.is_active(chat_id);

    if live_active {
        handle_text_live(bot, msg, state, &text).await
    } else {
        handle_text_oneshot(bot, msg, state, &text).await
    }
}

async fn handle_text_live(
    bot: Bot,
    msg: Message,
    state: AppState,
    text: &str,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0;

    // Send prompt to persistent RPC process
    {
        let mut live = state.live_sessions.lock().await;
        if let Err(e) = live.send_prompt(chat_id, text).await {
            bot.send_message(msg.chat.id, format!("Live mode error: {e}"))
                .await?;
            return Ok(());
        }
    }

    // Send initial status
    let status_msg = bot
        .send_message(msg.chat.id, "\u{1f534} LIVE | Working...")
        .await?;

    // Keep typing indicator active
    let bot_typing = bot.clone();
    let chat_id_tg = msg.chat.id;
    let typing_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(4));
        loop {
            interval.tick().await;
            let _ = bot_typing
                .send_chat_action(chat_id_tg, ChatAction::Typing)
                .await;
        }
    });

    // Accumulate response text from events
    let mut accumulated_text = String::new();
    let mut last_status_update = Instant::now();
    let mut current_status = "\u{1f534} LIVE | Working...".to_string();

    loop {
        let event = {
            let mut live = state.live_sessions.lock().await;
            // Use a timeout so we don't hold the lock forever
            tokio::time::timeout(Duration::from_millis(100), live.recv_event(chat_id)).await
        };

        match event {
            Ok(Some(PiEvent::TextDelta(delta))) => {
                accumulated_text.push_str(&delta);

                // Throttle status updates
                if last_status_update.elapsed() > Duration::from_secs(2) {
                    let preview = if accumulated_text.len() > 100 {
                        format!("{}...", &accumulated_text[..100])
                    } else {
                        accumulated_text.clone()
                    };
                    let new_status = format!("\u{1f534} LIVE | \u{270d}\u{fe0f} {preview}");
                    if new_status != current_status {
                        let _ = bot
                            .edit_message_text(msg.chat.id, status_msg.id, &new_status)
                            .await;
                        current_status = new_status;
                        last_status_update = Instant::now();
                    }
                }
            }
            Ok(Some(PiEvent::ToolStart { name })) => {
                let new_status =
                    format!("\u{1f534} LIVE | \u{26a1} Running {name}...");
                if new_status != current_status {
                    let _ = bot
                        .edit_message_text(msg.chat.id, status_msg.id, &new_status)
                        .await;
                    current_status = new_status;
                    last_status_update = Instant::now();
                }
            }
            Ok(Some(PiEvent::AgentEnd)) => break,
            Ok(Some(PiEvent::Error(e))) => {
                typing_handle.abort();
                let _ = bot.delete_message(msg.chat.id, status_msg.id).await;
                bot.send_message(msg.chat.id, format!("Error: {e}"))
                    .await?;
                return Ok(());
            }
            Ok(Some(_)) => {} // Ignore other events
            Ok(None) => break,  // Channel closed
            Err(_) => continue, // Timeout, try again
        }
    }

    typing_handle.abort();

    // Delete status message
    let _ = bot.delete_message(msg.chat.id, status_msg.id).await;

    // Send accumulated response
    if !accumulated_text.is_empty() {
        let chunks = split_message(accumulated_text.trim());
        for chunk in chunks {
            match bot
                .send_message(msg.chat.id, markdown_to_html(&chunk))
                .parse_mode(teloxide::types::ParseMode::Html)
                .await
            {
                Ok(_) => {}
                Err(_) => {
                    bot.send_message(msg.chat.id, strip_markdown(&chunk))
                        .await?;
                }
            }
        }
    }

    Ok(())
}

async fn handle_text_oneshot(
    bot: Bot,
    msg: Message,
    state: AppState,
    text: &str,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0;

    let workspace = state
        .workspace_mgr
        .lock()
        .await
        .get_workspace(chat_id)
        .await;
    let workspace_str = workspace.to_string_lossy().to_string();

    // Snapshot workspace before execution
    let before_snapshot = snapshot_workspace(&workspace).await;

    // Track session line count for image extraction
    let session_lines_before = get_session_line_count(&state.config, chat_id).await;

    // Send initial status message
    let status_msg = bot
        .send_message(msg.chat.id, "\u{1f504} Working...")
        .await?;

    let last_status_update = Arc::new(std::sync::Mutex::new(Instant::now()));

    // Activity emoji mapping
    let activity_emoji = |t: &ActivityType| -> &str {
        match t {
            ActivityType::Thinking => "\u{1f9e0}",
            ActivityType::Reading => "\u{1f4d6}",
            ActivityType::Writing => "\u{270d}\u{fe0f}",
            ActivityType::Running => "\u{26a1}",
            ActivityType::Searching => "\u{1f50d}",
            ActivityType::Working => "\u{1f504}",
        }
    };

    // Activity callback
    let bot_cb = bot.clone();
    let chat_id_tg = msg.chat.id;
    let status_msg_id = status_msg.id;
    let last_update = last_status_update.clone();

    let on_activity = move |activity: ActivityUpdate| {
        let mut last = last_update.lock().unwrap();
        if last.elapsed() < Duration::from_secs(2) {
            return;
        }
        *last = Instant::now();

        let emoji = activity_emoji(&activity.activity_type);
        let detail = if activity.detail.is_empty() {
            String::new()
        } else {
            format!("\n\u{2514}\u{2500} {}", activity.detail)
        };
        let text = format!("{emoji} Working... ({}s){detail}", activity.elapsed);

        let bot_inner = bot_cb.clone();
        tokio::spawn(async move {
            let _ = bot_inner
                .edit_message_text(chat_id_tg, status_msg_id, text)
                .await;
        });
    };

    // Keep typing indicator active
    let bot_typing = bot.clone();
    let typing_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(4));
        loop {
            interval.tick().await;
            let _ = bot_typing
                .send_chat_action(chat_id_tg, ChatAction::Typing)
                .await;
        }
    });

    // Acquire lock and run Pi
    let _guard = state.chat_locks.acquire(chat_id).await;

    let result = run_pi_with_streaming(
        &state.config,
        chat_id,
        text,
        &workspace_str,
        on_activity,
        None,
    )
    .await;

    typing_handle.abort();

    // Delete status message
    let _ = bot.delete_message(msg.chat.id, status_msg.id).await;

    // Send error if any
    if let Some(ref error) = result.error {
        bot.send_message(msg.chat.id, format!("Error: {error}"))
            .await?;
    }

    // Send output
    if !result.output.is_empty() {
        let chunks = split_message(result.output.trim());
        for chunk in chunks {
            match bot
                .send_message(msg.chat.id, markdown_to_html(&chunk))
                .parse_mode(teloxide::types::ParseMode::Html)
                .await
            {
                Ok(_) => {}
                Err(_) => {
                    bot.send_message(msg.chat.id, strip_markdown(&chunk))
                        .await?;
                }
            }
        }
    }

    // Extract and send tool images from session
    let tool_images =
        extract_images_from_session(&state.config, chat_id, session_lines_before).await;

    for (i, img) in tool_images.iter().enumerate() {
        let ext = img
            .mime_type
            .split('/')
            .nth(1)
            .unwrap_or("png");
        let filename = format!("image_{}.{ext}", i + 1);
        let input = InputFile::memory(img.data.clone()).file_name(filename);
        if let Err(e) = bot.send_photo(msg.chat.id, input).await {
            tracing::error!("Failed to send tool image: {e}");
        }
    }

    // Detect and send new files from workspace
    let detected_files = detect_files(&result.output, &workspace, &before_snapshot).await;

    for file in detected_files {
        let path_str = file.path.to_string_lossy().to_string();
        let send_result = match file.file_type {
            crate::file_detector::DetectedFileType::Photo => {
                bot.send_photo(msg.chat.id, InputFile::file(&path_str))
                    .caption(&file.filename)
                    .await
                    .map(|_| ())
            }
            crate::file_detector::DetectedFileType::Document => {
                bot.send_document(msg.chat.id, InputFile::file(&path_str))
                    .caption(&file.filename)
                    .await
                    .map(|_| ())
            }
        };
        if send_result.is_err() {
            bot.send_message(
                msg.chat.id,
                format!("(Could not send file: {})", file.filename),
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn handle_photo(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    // Access control
    if !state.check_access(&msg) {
        bot.send_message(msg.chat.id, "Sorry, you are not authorized to use this bot.")
            .await?;
        return Ok(());
    }

    let chat_id = msg.chat.id.0;

    // Rate limiting
    {
        let mut limiter = state.rate_limiter.lock().await;
        let result = limiter.check(chat_id, state.config.rate_limit_cooldown_ms);
        if !result.allowed {
            let secs = result.retry_after_ms.unwrap_or(0).div_ceil(1000);
            bot.send_message(
                msg.chat.id,
                format!("Please wait {secs}s before sending another message."),
            )
            .await?;
            return Ok(());
        }
    }

    let Some(photos) = msg.photo() else {
        return Ok(());
    };

    let caption = msg
        .caption()
        .unwrap_or("What's in this image?")
        .to_string();

    // Get largest photo (last in array)
    let largest = &photos[photos.len() - 1];

    // Download photo
    let file = bot.get_file(largest.file.id.clone()).await?;
    let file_path = file.path;

    let file_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        state.config.telegram_token, file_path
    );

    let response = reqwest::get(&file_url).await?;
    let image_bytes = response.bytes().await?;

    // Save to temp file
    let ext = file_path.split('.').next_back().unwrap_or("jpg");
    let temp_dir = std::env::temp_dir().join("mini-claw");
    tokio::fs::create_dir_all(&temp_dir).await?;
    let temp_path = temp_dir.join(format!("{chat_id}-{}.{ext}", chrono_timestamp()));
    tokio::fs::write(&temp_path, &image_bytes).await?;

    // Send status
    let status_msg = bot
        .send_message(msg.chat.id, "\u{1f504} Analyzing image...")
        .await?;

    let workspace = state
        .workspace_mgr
        .lock()
        .await
        .get_workspace(chat_id)
        .await;
    let workspace_str = workspace.to_string_lossy().to_string();

    let before_snapshot = snapshot_workspace(&workspace).await;

    // Activity callback (simplified for photo)
    let bot_cb = bot.clone();
    let chat_id_tg = msg.chat.id;
    let status_msg_id = status_msg.id;
    let last_update = Arc::new(std::sync::Mutex::new(Instant::now()));

    let on_activity = move |activity: ActivityUpdate| {
        let mut last = last_update.lock().unwrap();
        if last.elapsed() < Duration::from_secs(2) {
            return;
        }
        *last = Instant::now();

        let emoji = match activity.activity_type {
            ActivityType::Thinking => "\u{1f914}",
            ActivityType::Reading => "\u{1f4d6}",
            ActivityType::Writing => "\u{270d}\u{fe0f}",
            ActivityType::Running => "\u{2699}\u{fe0f}",
            ActivityType::Searching => "\u{1f50d}",
            ActivityType::Working => "\u{1f504}",
        };
        let detail = if activity.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", activity.detail)
        };
        let elapsed = if activity.elapsed > 0 {
            format!(" ({}s)", activity.elapsed)
        } else {
            String::new()
        };
        let text = format!("{emoji}{detail}{elapsed}");

        let bot_inner = bot_cb.clone();
        tokio::spawn(async move {
            let _ = bot_inner
                .edit_message_text(chat_id_tg, status_msg_id, text)
                .await;
        });
    };

    // Acquire lock and run Pi with image
    let _guard = state.chat_locks.acquire(chat_id).await;

    let options = RunPiOptions {
        image_paths: vec![temp_path.clone()],
    };

    let result = run_pi_with_streaming(
        &state.config,
        chat_id,
        &caption,
        &workspace_str,
        on_activity,
        Some(options),
    )
    .await;

    // Delete status
    let _ = bot.delete_message(msg.chat.id, status_msg.id).await;

    // Send error if any
    if let Some(ref error) = result.error {
        bot.send_message(msg.chat.id, format!("Error: {error}"))
            .await?;
    }

    // Send output
    if !result.output.is_empty() {
        let chunks = split_message(result.output.trim());
        for chunk in chunks {
            match bot
                .send_message(msg.chat.id, markdown_to_html(&chunk))
                .parse_mode(teloxide::types::ParseMode::Html)
                .await
            {
                Ok(_) => {}
                Err(_) => {
                    bot.send_message(msg.chat.id, strip_markdown(&chunk))
                        .await?;
                }
            }
        }
    }

    // Detect and send files
    let detected_files = detect_files(&result.output, &workspace, &before_snapshot).await;
    for file in detected_files {
        let path_str = file.path.to_string_lossy().to_string();
        match file.file_type {
            crate::file_detector::DetectedFileType::Photo => {
                let _ = bot
                    .send_photo(msg.chat.id, InputFile::file(&path_str))
                    .caption(&file.filename)
                    .await;
            }
            crate::file_detector::DetectedFileType::Document => {
                let _ = bot
                    .send_document(msg.chat.id, InputFile::file(&path_str))
                    .caption(&file.filename)
                    .await;
            }
        }
    }

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    Ok(())
}

fn chrono_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

