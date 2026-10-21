-- Cartes citées dans l'effet (`refs`), `plain` par défaut.
ALTER TABLE cards ADD COLUMN refs TEXT NOT NULL DEFAULT 'plain';
