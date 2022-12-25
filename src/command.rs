pub mod argument;

use poise::serenity_prelude as serenity;

pub type Context<'a> = poise::Context<'a, Data, eyre::Report>;

pub struct Data {
    pub db: sea_orm::DbConn,
    pub train_guild_id: serenity::GuildId,
}

#[poise::command(prefix_command, owners_only)]
pub async fn delete_message(ctx: Context<'_>, channel_id: u64, msg_id: u64) -> eyre::Result<()> {
    serenity::ChannelId(channel_id)
        .delete_message(ctx.discord(), msg_id)
        .await?;
    ctx.say("Message deleted.").await?;
    Ok(())
}

#[poise::command(prefix_command)]
pub async fn hello(ctx: Context<'_>) -> eyre::Result<()> {
    ctx.say("Hi! I wish only to hear your words, share your feelings, hear your thoughts.")
        .await?;
    Ok(())
}

pub fn all() -> Vec<poise::Command<Data, eyre::Report>> {
    vec![crate::train::command::train(), hello(), delete_message()]
}
