-- Taille de la carte (`size`), `normal` par défaut.
ALTER TABLE cards ADD COLUMN size TEXT NOT NULL DEFAULT 'normal';
CREATE INDEX idx_cards_size ON cards (size);
