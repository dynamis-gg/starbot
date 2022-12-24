CREATE TABLE trains (
    guild_id INT NOT NULL,
    world TEXT NOT NULL,
    expac INT NOT NULL,
    channel_id INT NOT NULL,
    message_id INT NOT NULL,
    status TEXT NOT NULL,
    last_run INT,
    scout_map TEXT,
    PRIMARY KEY (guild_id, world, expac),
    UNIQUE (channel_id, message_id)
)