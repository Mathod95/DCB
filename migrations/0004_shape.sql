-- Style du contour de type (`shape`), `solid` par défaut.
ALTER TABLE cards ADD COLUMN shape TEXT NOT NULL DEFAULT 'solid';
CREATE INDEX idx_cards_shape ON cards (shape);
