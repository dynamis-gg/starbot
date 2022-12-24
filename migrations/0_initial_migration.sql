CREATE TABLE trains (
    id INT NOT NULL PRIMARY KEY,
    world TEXT NOT NULL,
    expac INT NOT NULL,
    status TEXT NOT NULL,
    last_run INT,
    scout_map TEXT,
    UNIQUE (world, expac)
);

CREATE TABLE monitors (
    id INT NOT NULL PRIMARY KEY,
    message_id INT NOT NULL,
    channel_id INT NOT NULL,
    train_id INT NOT NULL,
);