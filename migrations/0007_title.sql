-- Emplacement du nom (`title`), `block` par défaut.
ALTER TABLE cards ADD COLUMN title TEXT NOT NULL DEFAULT 'block';
