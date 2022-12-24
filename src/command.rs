pub mod argument;
pub mod train;

use poise::serenity_prelude as serenity;

type Context<'a> = poise::Context<'a, Data, eyre::Report>;

pub struct Data {
    pub db: sea_orm::DbConn,
    pub train_guild_id: serenity::GuildId,
}

#[poise::command(prefix_command)]
pub async fn hello(ctx: Context<'_>) -> eyre::Result<()> {
    ctx.say("Hi! I wish only to hear your words, share your feelings, hear your thoughts.")
        .await?;
    Ok(())
}

pub fn all() -> Vec<poise::Command<Data, eyre::Report>> {
    vec![train::train(), hello()]
}
