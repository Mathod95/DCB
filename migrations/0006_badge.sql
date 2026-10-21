-- Emplacement du type (`badge`), `text` par défaut.
ALTER TABLE cards ADD COLUMN badge TEXT NOT NULL DEFAULT 'text';
