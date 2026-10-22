-- Bloc Effect façon code (`effect_style`, `text` par défaut) et son nom de fichier.
ALTER TABLE cards ADD COLUMN effect_style TEXT NOT NULL DEFAULT 'text';
ALTER TABLE cards ADD COLUMN effect_file TEXT NOT NULL DEFAULT '';
