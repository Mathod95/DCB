-- Base persistante de l'app (data/app.db) : comptes et sessions. Contrairement au cache des
-- cartes, elle n'est jamais reconstruite : c'est elle qu'il faut sauvegarder en production.
CREATE TABLE users (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    username       TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash  TEXT    NOT NULL,
    role           TEXT    NOT NULL CHECK (role IN ('admin', 'user')),
    active         INTEGER NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    last_login_at  INTEGER
);

-- Seul le hash SHA-256 du jeton de session est stocké : une fuite de la base ne permet pas de
-- se faire passer pour un utilisateur connecté.
CREATE TABLE sessions (
    token_hash  TEXT    PRIMARY KEY NOT NULL,
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at  INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX idx_sessions_user ON sessions (user_id);
