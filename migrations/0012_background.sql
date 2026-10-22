-- Fond de la carte (`background`), `plain` par défaut.
ALTER TABLE cards ADD COLUMN background TEXT NOT NULL DEFAULT 'plain';
