CREATE TABLE trains (
    id INTEGER PRIMARY KEY,
    world TEXT NOT NULL,
    expac INTEGER NOT NULL,
    status INTEGER NOT NULL,
    last_run INTEGER,
    scout_map TEXT,
    UNIQUE (world, expac)
);

CREATE TABLE monitors (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    train_id INTEGER NOT NULL
);