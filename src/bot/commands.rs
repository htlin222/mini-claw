use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

use super::util::{run_shell, split_message};
use super::AppState;
use crate::pi_runner::check_pi_auth;
use crate::sessions::{
    archive_session, format_file_size, format_session_age,
    generate_session_title, list_sessions,
};
use crate::workspace::WorkspaceManager;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum BotCommand {
    #[command(description = "Welcome & quick start")]
    Start,
    #[command(description = "Show all commands")]
    Help,
    #[command(description = "Show current directory")]
    Pwd,
    #[command(description = "Change directory")]
    Cd(String),
    #[command(description = "Go to home directory")]
    Home,
    #[command(description = "Run shell command")]
    Shell(String),
    #[command(description = "Manage sessions")]
    Session,
    #[command(description = "Start fresh conversation")]
    New,
    #[command(description = "Show bot status")]
    Status,
    #[command(description = "Toggle live interactive mode")]
    Live(String),
}

pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: BotCommand,
    state: AppState,
) -> anyhow::Result<()> {
    // Access control
    if !state.check_access(&msg) {
        bot.send_message(msg.chat.id, "Sorry, you are not authorized to use this bot.")
            .await?;
        return Ok(());
    }

    match cmd {
        BotCommand::Start => handle_start(bot, msg, state).await,
        BotCommand::Help => handle_help(bot, msg).await,
        BotCommand::Pwd => handle_pwd(bot, msg, state).await,
        BotCommand::Cd(path) => handle_cd(bot, msg, state, &path).await,
        BotCommand::Home => handle_home(bot, msg, state).await,
        BotCommand::Shell(cmd) => handle_shell(bot, msg, state, &cmd).await,
        BotCommand::Session => handle_session(bot, msg, state).await,
        BotCommand::New => handle_new(bot, msg, state).await,
        BotCommand::Status => handle_status(bot, msg, state).await,
        BotCommand::Live(arg) => handle_live(bot, msg, state, &arg).await,
    }
}

async fn handle_start(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    let pi_ok = check_pi_auth().await;
    let status = if pi_ok {
        "Pi is ready"
    } else {
        "Pi is not installed or not authenticated"
    };

    let cwd = state.workspace_mgr.lock().await.get_workspace(msg.chat.id.0).await;
    let formatted = WorkspaceManager::format_path(&cwd);

    bot.send_message(
        msg.chat.id,
        format!(
            "Welcome to Mini-Claw!\n\n{status}\nWorking directory: {formatted}\n\nType /help for all commands.\nSend any message to chat with AI."
        ),
    )
    .await?;
    Ok(())
}

async fn handle_help(bot: Bot, msg: Message) -> anyhow::Result<()> {
    bot.send_message(
        msg.chat.id,
        "\u{1f4d6} Mini-Claw Commands\n\n\
        \u{1f4c1} Navigation:\n\
        /pwd - Show current directory\n\
        /cd <path> - Change directory\n\
        /home - Go to home directory\n\n\
        \u{1f527} Execution:\n\
        /shell <cmd> - Run shell command directly\n\n\
        \u{1f4ac} Sessions:\n\
        /session - List & manage sessions\n\
        /new - Archive current & start fresh\n\n\
        \u{1f4ca} Info:\n\
        /status - Show bot status\n\
        /help - Show this message\n\n\
        \u{1f517} Interactive:\n\
        /live - Toggle persistent Pi session\n\n\
        \u{1f4a1} Tips:\n\
        \u{2022} Any text \u{2192} AI conversation\n\
        \u{2022} /shell runs instantly, no AI\n\
        \u{2022} /cd supports ~, .., relative paths\n\
        \u{2022} /live enables mid-conversation interaction"
    )
    .await?;
    Ok(())
}

async fn handle_pwd(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    let cwd = state.workspace_mgr.lock().await.get_workspace(msg.chat.id.0).await;
    let formatted = WorkspaceManager::format_path(&cwd);
    bot.send_message(msg.chat.id, format!("\u{1f4c1} {formatted}"))
        .await?;
    Ok(())
}

async fn handle_cd(bot: Bot, msg: Message, state: AppState, path: &str) -> anyhow::Result<()> {
    let path = if path.trim().is_empty() { "~" } else { path.trim() };

    match state.workspace_mgr.lock().await.set_workspace(msg.chat.id.0, path).await {
        Ok(cwd) => {
            let formatted = WorkspaceManager::format_path(&cwd);
            bot.send_message(msg.chat.id, format!("\u{1f4c1} {formatted}"))
                .await?;
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("Error: {e}"))
                .await?;
        }
    }
    Ok(())
}

async fn handle_home(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    handle_cd(bot, msg, state, "~").await
}

async fn handle_shell(
    bot: Bot,
    msg: Message,
    state: AppState,
    cmd: &str,
) -> anyhow::Result<()> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        bot.send_message(msg.chat.id, "Usage: /shell <command>\nExample: /shell ls -la")
            .await?;
        return Ok(());
    }

    let cwd = state.workspace_mgr.lock().await.get_workspace(msg.chat.id.0).await;
    bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
        .await?;

    let result = run_shell(cmd, &cwd, state.config.shell_timeout_ms).await;

    let mut output = String::new();
    if !result.stdout.is_empty() {
        output.push_str(&result.stdout);
    }
    if !result.stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("stderr: ");
        output.push_str(&result.stderr);
    }
    if output.is_empty() {
        output = "(no output)".to_string();
    }

    if result.code != Some(0) {
        output.push_str(&format!("\n\n[exit code: {:?}]", result.code));
    }

    let chunks = split_message(output.trim());
    for chunk in chunks {
        bot.send_message(msg.chat.id, chunk).await?;
    }
    Ok(())
}

async fn handle_session(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
        .await?;

    let sessions = list_sessions(&state.config).await;

    if sessions.is_empty() {
        bot.send_message(msg.chat.id, "No sessions found.").await?;
        return Ok(());
    }

    // Generate titles for up to 10 sessions
    let mut sessions_with_titles = Vec::new();
    for session in sessions.iter().take(10) {
        let title =
            generate_session_title(&session.path, state.config.session_title_timeout_ms).await;
        sessions_with_titles.push((session, title));
    }

    // Build inline keyboard
    let mut keyboard = Vec::new();
    for (session, title) in &sessions_with_titles {
        let age = format_session_age(session.modified_at);
        let size = format_file_size(session.size_bytes);
        let label = format!("{title} ({age}, {size})");
        let callback_data = format!("session:load:{}", session.filename);
        keyboard.push(vec![teloxide::types::InlineKeyboardButton::callback(
            label,
            callback_data,
        )]);
    }

    // Add cleanup button
    keyboard.push(vec![teloxide::types::InlineKeyboardButton::callback(
        "\u{1f5d1} Clean Up Old Sessions",
        "session:cleanup",
    )]);

    let markup = teloxide::types::InlineKeyboardMarkup::new(keyboard);

    bot.send_message(
        msg.chat.id,
        format!(
            "\u{1f4da} Sessions ({} total)\n\nTap to switch session:",
            sessions.len()
        ),
    )
    .reply_markup(markup)
    .await?;

    Ok(())
}

async fn handle_new(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0;

    // Stop live session if active
    {
        let mut live = state.live_sessions.lock().await;
        if live.is_active(chat_id) {
            live.stop_session(chat_id).await;
        }
    }

    // Acquire lock to prevent concurrent Pi access
    let _guard = state.chat_locks.acquire(chat_id).await;

    let archived = archive_session(&state.config, chat_id).await;
    state.session_mgr.lock().await.clear_active_session(chat_id).await;

    let reply = if let Some(name) = archived {
        format!("Session archived as {name}\nStarting fresh conversation.")
    } else {
        "Starting fresh conversation.".to_string()
    };

    bot.send_message(msg.chat.id, reply).await?;
    Ok(())
}

async fn handle_status(bot: Bot, msg: Message, state: AppState) -> anyhow::Result<()> {
    let pi_ok = check_pi_auth().await;
    let cwd = state.workspace_mgr.lock().await.get_workspace(msg.chat.id.0).await;
    let formatted = WorkspaceManager::format_path(&cwd);
    let live_active = state.live_sessions.lock().await.is_active(msg.chat.id.0);

    bot.send_message(
        msg.chat.id,
        format!(
            "Status:\n\
            - Pi: {}\n\
            - Chat ID: {}\n\
            - Workspace: {formatted}\n\
            - Live mode: {}",
            if pi_ok { "OK" } else { "Not available" },
            msg.chat.id,
            if live_active { "ON" } else { "OFF" },
        ),
    )
    .await?;
    Ok(())
}

async fn handle_live(bot: Bot, msg: Message, state: AppState, arg: &str) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0;
    let arg = arg.trim().to_lowercase();

    let is_active = state.live_sessions.lock().await.is_active(chat_id);

    match arg.as_str() {
        "on" | "" if !is_active => {
            // Enable live mode
            let workspace = state.workspace_mgr.lock().await.get_workspace(chat_id).await;
            let session_path = state.config.session_dir.join(format!("telegram-{chat_id}.jsonl"));

            tokio::fs::create_dir_all(&state.config.session_dir).await?;

            match state.live_sessions.lock().await.start_session(
                chat_id,
                &session_path,
                &workspace,
                state.config.thinking_level,
            ).await {
                Ok(_) => {
                    bot.send_message(
                        msg.chat.id,
                        "\u{1f534} Live mode enabled!\nPi is now running persistently. Messages go directly to the active session.\nUse /live off to disable.",
                    )
                    .await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Failed to start live mode: {e}"))
                        .await?;
                }
            }
        }
        "off" | "" if is_active => {
            state.live_sessions.lock().await.stop_session(chat_id).await;
            bot.send_message(
                msg.chat.id,
                "Live mode disabled. Switched back to one-shot mode.",
            )
            .await?;
        }
        "status" => {
            let status = if is_active { "ON" } else { "OFF" };
            bot.send_message(msg.chat.id, format!("Live mode: {status}"))
                .await?;
        }
        "" => {
            // Already handled by the on/off branches above via is_active check
            unreachable!()
        }
        _ => {
            bot.send_message(
                msg.chat.id,
                "Usage: /live [on|off|status]\nNo argument toggles the mode.",
            )
            .await?;
        }
    }

    Ok(())
}

// This is used by pi_runner lock - re-export for use outside the module
