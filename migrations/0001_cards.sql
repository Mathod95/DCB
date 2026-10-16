-- Cartes de DevCardBattle. Les enums (card_type, skin, rarity) sont stockés en texte :
-- ajouter une valeur ne demande pas de migration, seulement une variante dans dcb-core.
CREATE TABLE cards (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT    NOT NULL,
    card_type    TEXT    NOT NULL,
    skin         TEXT    NOT NULL DEFAULT 'light',
    rarity       TEXT    NOT NULL DEFAULT 'basic',
    description  TEXT    NOT NULL DEFAULT '',
    effect       TEXT    NOT NULL DEFAULT '',
    deploy_cost  INTEGER NOT NULL DEFAULT 0 CHECK (deploy_cost BETWEEN 0 AND 99),
    daily_cost   INTEGER NOT NULL DEFAULT 0 CHECK (daily_cost BETWEEN 0 AND 99),
    flavor_text  TEXT    NOT NULL DEFAULT '',
    art_path     TEXT,
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_cards_type ON cards (card_type);
