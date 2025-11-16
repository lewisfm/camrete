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
    module_versions (repo_url, module_id, version) {
        repo_url -> Text,
        module_id -> Text,
        version -> Text,
        name -> Text,
        summary -> Text,
    }
}

diesel::table! {
    modules (repo_url, id) {
        repo_url -> Text,
        id -> Text,
        download_count -> Integer,
    }
}

diesel::table! {
    repositories (url) {
        url -> Text,
        name -> Text,
        priority -> Integer,
        x_mirror -> Bool,
        x_comment -> Nullable<Text>,
    }
}

diesel::table! {
    repository_refs (owner_url, url) {
        owner_url -> Text,
        name -> Text,
        url -> Text,
        priority -> Integer,
        x_mirror -> Integer,
        x_comment -> Nullable<Text>,
    }
}

diesel::joinable!(module_versions -> repositories (repo_url));
diesel::joinable!(modules -> repositories (repo_url));

diesel::allow_tables_to_appear_in_same_query!(
    builds,
    etags,
    module_versions,
    modules,
    repositories,
    repository_refs,
);
