CREATE TABLE organizations (
    id uuid PRIMARY KEY,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    version bigint NOT NULL DEFAULT 1 CHECK (version > 0)
);
