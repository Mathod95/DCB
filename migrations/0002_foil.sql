-- Finition holographique des cartes (none, stars, glitter, shards, rainbow).
ALTER TABLE cards ADD COLUMN foil TEXT NOT NULL DEFAULT 'none';
