-- La rareté devient une finition : `gold` et `basic` sont des valeurs de `foil`.
-- Le cache est reconstruit depuis cards/*.toml au démarrage, ces lignes ne servent qu'à
-- garder une base existante cohérente.
UPDATE cards SET foil = 'gold' WHERE rarity = 'gold' AND foil = 'none';
UPDATE cards SET foil = 'basic' WHERE foil = 'none';
ALTER TABLE cards DROP COLUMN rarity;
ALTER TABLE cards RENAME TO cards_old;
CREATE TABLE cards (
    slug         TEXT    PRIMARY KEY NOT NULL,
    name         TEXT    NOT NULL,
    card_type    TEXT    NOT NULL,
    skin         TEXT    NOT NULL,
    foil         TEXT    NOT NULL DEFAULT 'basic',
    description  TEXT    NOT NULL DEFAULT '',
    effect       TEXT    NOT NULL DEFAULT '',
    deploy_cost  INTEGER NOT NULL DEFAULT 0,
    daily_cost   INTEGER NOT NULL DEFAULT 0,
    flavor_text  TEXT    NOT NULL DEFAULT '',
    art          TEXT
) WITHOUT ROWID;
INSERT INTO cards SELECT slug, name, card_type, skin, foil, description, effect,
                         deploy_cost, daily_cost, flavor_text, art FROM cards_old;
DROP TABLE cards_old;
CREATE INDEX idx_cards_type ON cards (card_type);
CREATE INDEX idx_cards_foil ON cards (foil);
