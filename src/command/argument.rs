use chrono::{DateTime, TimeZone, Utc};
use eyre::eyre;
use poise::serenity_prelude as serenity;
use poise::SlashArgument;

pub struct Timestamp(pub DateTime<Utc>);

#[derive(Debug)]
struct ErrWrap(eyre::Report);

impl std::fmt::Display for ErrWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ErrWrap {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[poise::async_trait]
impl SlashArgument for Timestamp {
    async fn extract(
        _ctx: &serenity::Context,
        _interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let str = value
            .as_str()
            .ok_or(poise::SlashArgError::CommandStructureMismatch(
                "expected string",
            ))?;
        // We accept discord-formatted timestamps, or more accurately, anything between two colons.
        // Also unformatted timestamps.
        let split = str.split(":");
        let timestamp = match *split.collect::<Vec<_>>() {
            [t] => t,
            [_, t, _] => t,
            _ => {
                return Err(poise::SlashArgError::Parse {
                    error: Box::new(ErrWrap(eyre!("expected Discord timestamp"))),
                    input: str.to_owned(),
                })
            }
        };
        match Utc.timestamp_opt(
            timestamp.parse().map_err(|e| poise::SlashArgError::Parse {
                error: Box::new(e),
                input: str.to_owned(),
            })?,
            0,
        ) {
            chrono::LocalResult::None => Err(poise::SlashArgError::Parse {
                error: Box::new(ErrWrap(eyre!("timestamp out of range"))),
                input: str.to_owned(),
            }),
            chrono::LocalResult::Single(t) => Ok(Timestamp(t)),
            chrono::LocalResult::Ambiguous(_, _) => panic!("this should never be ambiguous"),
        }
    }

    fn create(builder: &mut poise::serenity_prelude::CreateApplicationCommandOption) {
        builder.kind(poise::serenity_prelude::CommandOptionType::String);
    }

    fn choices() -> Vec<poise::CommandParameterChoice> {
        Vec::new()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<t:{}:f>", self.0.timestamp())
    }
}
