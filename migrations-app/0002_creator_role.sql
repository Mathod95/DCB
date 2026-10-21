-- Rôle « creator » (crée des cartes et modifie les siennes) entre « user » (joueur) et « admin ».
-- SQLite ne modifie pas une contrainte CHECK : la table des comptes est reconstruite. Les
-- sessions sont mises de côté puis remises, pour que personne ne soit déconnecté.
CREATE TABLE sessions_keep AS SELECT token_hash, user_id, created_at, expires_at FROM sessions;
DROP TABLE sessions;

CREATE TABLE users_new (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    username       TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash  TEXT    NOT NULL,
    role           TEXT    NOT NULL CHECK (role IN ('admin', 'creator', 'user')),
    active         INTEGER NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    last_login_at  INTEGER
);
INSERT INTO users_new (id, username, password_hash, role, active, created_at, last_login_at)
    SELECT id, username, password_hash, role, active, created_at, last_login_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE TABLE sessions (
    token_hash  TEXT    PRIMARY KEY NOT NULL,
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at  INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO sessions SELECT * FROM sessions_keep;
DROP TABLE sessions_keep;
CREATE INDEX idx_sessions_user ON sessions (user_id);
