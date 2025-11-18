use diesel::{table, joinable, allow_tables_to_appear_in_same_query};

table! {
    builds (build_id) {
        build_id -> Integer,
        version -> Binary,
    }
}

table! {
    etags (url) {
        url -> Binary,
        etag -> Text,
    }
}

table! {
    module_authors (id) {
        id -> Integer,
        release_id -> Integer,
        ordinal -> Integer,
        author -> Text,
    }
}

table! {
    module_licenses (id) {
        id -> Integer,
        release_id -> Integer,
        license -> Text,
    }
}

table! {
    module_localizations (id) {
        id -> Integer,
        release_id -> Integer,
        locale -> Text,
    }
}

table! {
    module_relationship_groups (group_id) {
        group_id -> Integer,
        release_id -> Integer,
        ordinal -> Integer,
        rel_type -> Integer,
        choice_help_text -> Nullable<Text>,
        suppress_recommendations -> Integer,
    }
}

table! {
    module_relationships (relationship_id) {
        relationship_id -> Integer,
        group_id -> Integer,
        ordinal -> Integer,
        target_name -> Text,
        target_version -> Nullable<Text>,
        target_version_min -> Nullable<Text>,
    }
}

table! {
    module_releases (release_id) {
        release_id -> Integer,
        module_id -> Integer,
        version -> Text,
        sort_index -> Integer,
        summary -> Text,
        metadata -> Binary,
        description -> Nullable<Text>,
        release_status -> Integer,
        game_version -> Binary,
        game_version_min -> Binary,
        game_version_strict -> Bool,
        download_size -> Nullable<BigInt>,
        install_size -> Nullable<BigInt>,
        release_date -> Nullable<TimestamptzSqlite>,
        kind -> Integer,
    }
}

table! {
    module_replacements (replacement_id) {
        replacement_id -> Integer,
        release_id -> Integer,
        target_name -> Text,
        target_version -> Nullable<Text>,
        target_version_min -> Nullable<Text>,
    }
}

table! {
    module_tags (id) {
        id -> Integer,
        release_id -> Integer,
        ordinal -> Integer,
        tag -> Text,
    }
}

table! {
    modules (module_id) {
        module_id -> Integer,
        repo_id -> Integer,
        module_name -> Text,
        download_count -> Integer,
    }
}

table! {
    repositories (repo_id) {
        repo_id -> Integer,
        url -> Binary,
        name -> Text,
        priority -> Integer,
        x_mirror -> Bool,
        x_comment -> Nullable<Text>,
    }
}

table! {
    repository_refs (referrer_repo_url, url) {
        referrer_repo_url -> Binary,
        name -> Text,
        url -> Binary,
        priority -> Integer,
        x_mirror -> Integer,
        x_comment -> Nullable<Text>,
    }
}

joinable!(module_authors -> module_releases (release_id));
joinable!(module_licenses -> module_releases (release_id));
joinable!(module_localizations -> module_releases (release_id));
joinable!(module_relationship_groups -> module_releases (release_id));
joinable!(module_relationships -> module_relationship_groups (group_id));
joinable!(module_releases -> modules (module_id));
joinable!(module_replacements -> module_releases (release_id));
joinable!(module_tags -> module_releases (release_id));
joinable!(modules -> repositories (repo_id));

allow_tables_to_appear_in_same_query!(
    builds,
    etags,
    module_authors,
    module_licenses,
    module_localizations,
    module_relationship_groups,
    module_relationships,
    module_releases,
    module_replacements,
    module_tags,
    modules,
    repositories,
    repository_refs,
);
