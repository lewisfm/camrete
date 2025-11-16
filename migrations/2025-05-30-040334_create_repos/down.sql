-- Drop in reverse order to avoid foreign key constraint errors

DROP TABLE IF EXISTS repository_refs;
DROP TABLE IF EXISTS etags;
DROP TABLE IF EXISTS builds;
DROP TABLE IF EXISTS module_replacements;
DROP TABLE IF EXISTS module_relationships;
DROP TABLE IF EXISTS module_relationship_groups;
DROP TABLE IF EXISTS module_localizations;
DROP TABLE IF EXISTS module_tags;
DROP TABLE IF EXISTS module_localizations;
DROP TABLE IF EXISTS module_licenses;
DROP TABLE IF EXISTS module_authors;
DROP TABLE IF EXISTS module_releases;
DROP TABLE IF EXISTS modules;
DROP TABLE IF EXISTS repositories;
