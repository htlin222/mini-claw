use teloxide::prelude::*;

use super::AppState;
use crate::sessions::cleanup_old_sessions;

pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: AppState,
) -> anyhow::Result<()> {
    let Some(data) = q.data.as_deref() else {
        return Ok(());
    };

    let data = data.to_string();
    if let Some(filename) = data.strip_prefix("session:load:") {
        handle_session_load(bot, q, state, filename).await
    } else if data == "session:cleanup" {
        handle_session_cleanup(bot, q, state).await
    } else {
        Ok(())
    }
}

async fn handle_session_load(
    bot: Bot,
    q: CallbackQuery,
    state: AppState,
    filename: &str,
) -> anyhow::Result<()> {
    let Some(msg) = q.message else {
        bot.answer_callback_query(q.id.clone())
            .text("Error: No message")
            .await?;
        return Ok(());
    };

    let chat_id = msg.chat().id.0;

    match state
        .session_mgr
        .lock()
        .await
        .switch_session(&state.config, chat_id, filename)
        .await
    {
        Ok(_) => {
            bot.answer_callback_query(q.id.clone())
                .text("Session switched!")
                .await?;
            let _ = bot
                .edit_message_text(
                    msg.chat().id,
                    msg.id(),
                    format!("\u{2705} Switched to session: {filename}"),
                )
                .await;
        }
        Err(e) => {
            bot.answer_callback_query(q.id.clone())
                .text(format!("Error: {e}"))
                .show_alert(true)
                .await?;
        }
    }

    Ok(())
}

async fn handle_session_cleanup(
    bot: Bot,
    q: CallbackQuery,
    state: AppState,
) -> anyhow::Result<()> {
    bot.answer_callback_query(q.id.clone())
        .text("Cleaning up...")
        .await?;

    let deleted = cleanup_old_sessions(&state.config, 5).await;

    if let Some(msg) = q.message {
        let _ = bot
            .edit_message_text(
                msg.chat().id,
                msg.id(),
                format!(
                    "\u{1f5d1} Cleanup complete!\nDeleted {deleted} old session(s).\nKept the 5 most recent sessions per chat."
                ),
            )
            .await;
    }

    Ok(())
}
