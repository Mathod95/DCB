-- Présentation de la description (`desc_style`), `classic` par défaut.
ALTER TABLE cards ADD COLUMN desc_style TEXT NOT NULL DEFAULT 'classic';
