CREATE TABLE trains (
    guild_id INT NOT NULL,
    world TEXT NOT NULL,
    expac TEXT NOT NULL,
    channel_id INT NOT NULL,
    message_id INT NOT NULL,
    status INT NOT NULL,
    last_run INT,
    scout_link TEXT,
    PRIMARY KEY (guild_id, world, expac)
)