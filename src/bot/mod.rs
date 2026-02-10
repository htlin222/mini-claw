pub mod callbacks;
pub mod commands;
pub mod handlers;
pub mod util;

use std::sync::Arc;

use teloxide::dispatching::UpdateFilterExt;
use teloxide::prelude::*;

use crate::config::Config;
use crate::pi_rpc::LiveSessionManager;
use crate::pi_runner::ChatLocks;
use crate::rate_limiter::RateLimiter;
use crate::sessions::SessionManager;
use crate::workspace::WorkspaceManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub rate_limiter: Arc<tokio::sync::Mutex<RateLimiter>>,
    pub workspace_mgr: Arc<tokio::sync::Mutex<WorkspaceManager>>,
    pub session_mgr: Arc<tokio::sync::Mutex<SessionManager>>,
    pub chat_locks: Arc<ChatLocks>,
    pub live_sessions: Arc<tokio::sync::Mutex<LiveSessionManager>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            rate_limiter: Arc::new(tokio::sync::Mutex::new(RateLimiter::new())),
            workspace_mgr: Arc::new(tokio::sync::Mutex::new(WorkspaceManager::new())),
            session_mgr: Arc::new(tokio::sync::Mutex::new(SessionManager::new())),
            chat_locks: Arc::new(ChatLocks::new()),
            live_sessions: Arc::new(tokio::sync::Mutex::new(LiveSessionManager::new())),
        }
    }

    pub fn check_access(&self, msg: &Message) -> bool {
        if self.config.allowed_users.is_empty() {
            return true;
        }
        msg.from
            .as_ref()
            .map(|user| self.config.allowed_users.contains(&(user.id.0 as i64)))
            .unwrap_or(false)
    }
}

pub async fn build_and_run(bot: Bot, state: AppState) {
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<commands::BotCommand>()
                        .endpoint(commands::handle_command),
                )
                .branch(
                    dptree::filter(|msg: Message| msg.photo().is_some())
                        .endpoint(handlers::handle_photo),
                )
                .branch(
                    dptree::filter(|msg: Message| msg.text().is_some())
                        .endpoint(handlers::handle_text),
                ),
        )
        .branch(
            Update::filter_callback_query().endpoint(callbacks::handle_callback),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
