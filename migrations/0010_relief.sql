-- Relief des blocs (`relief`), `flat` par défaut.
ALTER TABLE cards ADD COLUMN relief TEXT NOT NULL DEFAULT 'flat';
