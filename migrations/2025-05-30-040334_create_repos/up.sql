CREATE TABLE repositories (
    repo_id INTEGER PRIMARY KEY NOT NULL,
    name TEXT UNIQUE NOT NULL,
    url BLOB NOT NULL,
    priority INTEGER NOT NULL,
    x_mirror BOOLEAN NOT NULL,
    x_comment TEXT
);

CREATE TABLE modules (
    module_id INTEGER PRIMARY KEY NOT NULL,
    repo_id INTEGER NOT NULL REFERENCES repositories(repo_id) ON DELETE CASCADE,
    module_name TEXT NOT NULL,

    download_count INTEGER NOT NULL DEFAULT 0,

    UNIQUE (repo_id, module_name)
);

CREATE INDEX idx_modules_repo_id ON modules(repo_id);

-- Module releases & relationships

CREATE TABLE module_releases (
    -- Required fields
    release_id INTEGER PRIMARY KEY NOT NULL,
    module_id INTEGER NOT NULL REFERENCES modules(module_id) ON DELETE CASCADE,
    version TEXT NOT NULL,

    sort_index INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL, -- aka abstract
    -- authors BLOB NOT NULL, -- string[]
    metadata BLOB NOT NULL,
    -- licenses BLOB NOT NULL, -- string[]

    -- Optional fields
    description TEXT,
    release_status INTEGER NOT NULL DEFAULT 0,
    game_version BLOB NOT NULL, -- or the max, if below is present
    game_version_min BLOB NOT NULL,
    game_version_strict INTEGER NOT NULL DEFAULT FALSE,
    -- tags BLOB, -- string[]
    -- localizations BLOB, -- string[]
    download_size INTEGER,
    install_size INTEGER,
    release_date TEXT,
    kind INTEGER NOT NULL DEFAULT 0,

    UNIQUE (module_id, version)
);

CREATE INDEX idx_module_releases_module_id ON module_releases(module_id);
CREATE INDEX idx_module_releases_module_id_sort_index ON module_releases(module_id, sort_index);

CREATE TABLE module_authors (
    id INTEGER PRIMARY KEY NOT NULL,
    release_id INTEGER NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    author TEXT NOT NULL,

    UNIQUE (release_id, ordinal)
);

CREATE INDEX idx_module_authors_release_id ON module_authors(release_id);

CREATE TABLE module_licenses (
    id INTEGER PRIMARY KEY NOT NULL,
    release_id INTEGER NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,
    license TEXT NOT NULL,

    UNIQUE (release_id, license)
);

CREATE INDEX idx_module_licenses_release_id ON module_licenses(release_id);

CREATE TABLE module_tags (
    id INTEGER PRIMARY KEY NOT NULL,
    release_id INTEGER NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    tag TEXT NOT NULL,

    UNIQUE (release_id, tag),
    UNIQUE (release_id, ordinal)
);

CREATE INDEX idx_module_tags_release_id ON module_tags(release_id);

CREATE TABLE module_localizations (
    id INTEGER PRIMARY KEY NOT NULL,
    release_id INTEGER NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,
    locale TEXT NOT NULL,

    UNIQUE (release_id, locale)
);

CREATE INDEX idx_module_localizations_release_id ON module_localizations(release_id);

CREATE TABLE module_relationship_groups (
    group_id INTEGER PRIMARY KEY NOT NULL, -- Only one dependency per group needs to be satisfied
    release_id INTEGER NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,   -- group index in original metadata

    rel_type INTEGER NOT NULL, -- depends, recommends, suggests, conflicts, provides
    choice_help_text TEXT,
    suppress_recommendations INTEGER NOT NULL DEFAULT FALSE,

    UNIQUE (release_id, ordinal)
);

CREATE INDEX idx_module_relationship_groups_release_id ON module_relationship_groups(release_id);

CREATE TABLE module_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL,
    group_id INTEGER NOT NULL REFERENCES module_relationship_groups(group_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL, -- order within group

    target_name TEXT NOT NULL, -- module id or virtual id from "provides"
    target_version TEXT,
    target_version_min TEXT, -- if specified, target_version is the max

    UNIQUE (group_id, ordinal)
);

CREATE INDEX idx_module_relationships_group_id ON module_relationships(group_id);

-- For deprecated modules that have replacements
CREATE TABLE module_replacements (
    replacement_id INTEGER PRIMARY KEY NOT NULL,
    release_id INTEGER UNIQUE NOT NULL REFERENCES module_releases(release_id) ON DELETE CASCADE,

    target_name TEXT NOT NULL, -- module id, not virtual id
    target_version TEXT,
    target_version_min TEXT
);

CREATE INDEX idx_module_replacements_release_id ON module_replacements(release_id);


CREATE TABLE builds (
    build_id INTEGER PRIMARY KEY NOT NULL,
    version BLOB NOT NULL
);

CREATE TABLE etags (
    url BLOB PRIMARY KEY NOT NULL,
    etag TEXT
);

CREATE TABLE repository_refs (
    referrer_id BLOB NOT NULL REFERENCES repositories(repo_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    url BLOB NOT NULL,
    priority INTEGER NOT NULL,
    x_mirror INTEGER NOT NULL,
    x_comment TEXT,
    PRIMARY KEY (referrer_id, name)
);
