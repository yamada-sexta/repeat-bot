mod db;
mod urls;

use std::env;
use std::sync::Arc;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{debug, error, info};

use db::Database;
use urls::extract_and_normalize_urls;

struct DbKey;

impl TypeMapKey for DbKey {
    type Value = Arc<Database>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bots
        if msg.author.bot {
            debug!(author = %msg.author.name, "Ignoring bot message");
            return;
        }

        debug!(
            author = %msg.author.name,
            channel = %msg.channel_id,
            content = %msg.content,
            "Received message"
        );

        let urls = extract_and_normalize_urls(&msg.content);
        if urls.is_empty() {
            debug!("No URLs found in message");
            return;
        }

        info!(
            author = %msg.author.name,
            count = urls.len(),
            urls = ?urls,
            "Extracted and normalized URLs"
        );

        let data = ctx.data.read().await;
        let db = data.get::<DbKey>().expect("Database not found in context");
        let channel_id = msg.channel_id.get();
        let guild_id = msg.guild_id.map(|g| g.get()).unwrap_or(0);
        let author_id = msg.author.id.get();
        let author_name = &msg.author.name;
        let message_id = msg.id.get();

        let mut duplicates: Vec<String> = Vec::new();

        for normalized_url in &urls {
            match db.find_duplicate(guild_id, channel_id, normalized_url) {
                Ok(Some(prior)) => {
                    info!(
                        url = %normalized_url,
                        prior_author = %prior.author_name,
                        prior_author_id = prior.author_id,
                        current_author_id = author_id,
                        "Found duplicate link in DB"
                    );

                    let timestamp_display = prior
                        .timestamp
                        .map(|ts| format!(" on <t:{}:R>", ts))
                        .unwrap_or_default();

                    duplicates.push(format!(
                        "🔗 <{}>\n↳ Previously sent by **{}**{}",
                        normalized_url, prior.author_name, timestamp_display
                    ));
                }
                Ok(None) => {
                    debug!(url = %normalized_url, "No duplicate found");
                }
                Err(e) => {
                    error!(url = %normalized_url, "Database lookup error: {e}");
                }
            }

            // Always record the link (even if duplicate)
            match db.record_link(
                guild_id,
                channel_id,
                author_id,
                author_name,
                message_id,
                normalized_url,
            ) {
                Ok(()) => info!(url = %normalized_url, author = %author_name, "Recorded link"),
                Err(e) => error!(url = %normalized_url, "Database insert error: {e}"),
            }
        }

        if !duplicates.is_empty() {
            let reply = format!("♻️ **Repost detected!**\n{}", duplicates.join("\n"));

            if let Err(e) = msg.reply(&ctx.http, &reply).await {
                error!("Failed to send reply: {e}");
            }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Load .env file if present (not required)
    let _ = dotenvy::dotenv();

    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN env var");

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "repeat_bot.db".to_string());
    let database = Arc::new(Database::new(&db_path).expect("Failed to initialize database"));

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<DbKey>(database);
    }

    info!("Starting bot...");
    if let Err(e) = client.start().await {
        error!("Client error: {e}");
    }
}
