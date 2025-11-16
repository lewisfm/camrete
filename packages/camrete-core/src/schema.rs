// @generated automatically by Diesel CLI.

diesel::table! {
    builds (build_id) {
        build_id -> Integer,
        version -> Text,
    }
}

diesel::table! {
    etags (url) {
        url -> Text,
        etag -> Text,
    }
}

diesel::table! {
    module_authors (id) {
        id -> Nullable<Integer>,
        release_id -> Integer,
        ordinal -> Integer,
        author -> Text,
    }
}

diesel::table! {
    module_licenses (id) {
        id -> Nullable<Integer>,
        release_id -> Integer,
        license -> Text,
    }
}

diesel::table! {
    module_localizations (id) {
        id -> Nullable<Integer>,
        release_id -> Integer,
        locale -> Text,
    }
}

diesel::table! {
    module_relationship_groups (group_id) {
        group_id -> Nullable<Integer>,
        release_id -> Integer,
        ordinal -> Integer,
        rel_type -> Integer,
        choice_help_text -> Nullable<Text>,
        suppress_recommendations -> Integer,
    }
}

diesel::table! {
    module_relationships (relationship_id) {
        relationship_id -> Nullable<Integer>,
        group_id -> Integer,
        ordinal -> Integer,
        target_name -> Text,
        target_version -> Nullable<Text>,
        target_version_min -> Nullable<Text>,
    }
}

diesel::table! {
    module_releases (release_id) {
        release_id -> Nullable<Integer>,
        module_id -> Integer,
        version -> Text,
        sort_index -> Integer,
        summary -> Text,
        metadata -> Binary,
        description -> Nullable<Text>,
        release_status -> Nullable<Integer>,
        game_version -> Nullable<Text>,
        game_version_min -> Nullable<Text>,
        game_version_strict -> Integer,
        download_size -> Nullable<Integer>,
        download_content_type -> Nullable<Text>,
        install_size -> Nullable<Integer>,
        release_date -> Nullable<Text>,
        kind -> Integer,
    }
}

diesel::table! {
    module_replacements (replacement_id) {
        replacement_id -> Nullable<Integer>,
        release_id -> Integer,
        target_name -> Text,
        target_version -> Nullable<Text>,
        target_version_min -> Nullable<Text>,
    }
}

diesel::table! {
    module_tags (id) {
        id -> Nullable<Integer>,
        release_id -> Integer,
        ordinal -> Integer,
        tag -> Text,
    }
}

diesel::table! {
    modules (module_id) {
        module_id -> Nullable<Integer>,
        repo_id -> Integer,
        module_name -> Text,
        download_count -> Integer,
    }
}

diesel::table! {
    repositories (repo_id) {
        repo_id -> Nullable<Integer>,
        url -> Text,
        name -> Text,
        priority -> Integer,
        x_mirror -> Bool,
        x_comment -> Nullable<Text>,
    }
}

diesel::table! {
    repository_refs (referrer_repo_url, url) {
        referrer_repo_url -> Text,
        name -> Text,
        url -> Text,
        priority -> Integer,
        x_mirror -> Integer,
        x_comment -> Nullable<Text>,
    }
}

diesel::joinable!(module_authors -> module_releases (release_id));
diesel::joinable!(module_licenses -> module_releases (release_id));
diesel::joinable!(module_localizations -> module_releases (release_id));
diesel::joinable!(module_relationship_groups -> module_releases (release_id));
diesel::joinable!(module_relationships -> module_relationship_groups (group_id));
diesel::joinable!(module_releases -> modules (module_id));
diesel::joinable!(module_replacements -> module_releases (release_id));
diesel::joinable!(module_tags -> module_releases (release_id));
diesel::joinable!(modules -> repositories (repo_id));

diesel::allow_tables_to_appear_in_same_query!(
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
