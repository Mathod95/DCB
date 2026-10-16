-- Cache des cartes. La source de vérité est cards/*.toml : cette table est vidée et
-- reconstruite au démarrage. Les enums (card_type, skin, rarity) sont stockés en texte.
CREATE TABLE cards (
    slug         TEXT    PRIMARY KEY NOT NULL,
    name         TEXT    NOT NULL,
    card_type    TEXT    NOT NULL,
    skin         TEXT    NOT NULL,
    rarity       TEXT    NOT NULL,
    description  TEXT    NOT NULL DEFAULT '',
    effect       TEXT    NOT NULL DEFAULT '',
    deploy_cost  INTEGER NOT NULL DEFAULT 0,
    daily_cost   INTEGER NOT NULL DEFAULT 0,
    flavor_text  TEXT    NOT NULL DEFAULT '',
    art          TEXT
) WITHOUT ROWID;

CREATE INDEX idx_cards_type ON cards (card_type);
