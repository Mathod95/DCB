-- Version de la carte (`version`), `v1` par défaut.
ALTER TABLE cards ADD COLUMN version TEXT NOT NULL DEFAULT 'v1';
