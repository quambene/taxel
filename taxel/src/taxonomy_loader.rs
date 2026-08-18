use crate::ebilanz::{taxonomy_version_from_schema_refs, TaxonomyType, TAXONOMY_VERSION_TO_DATE};
use anyhow::Context;
use log::debug;
use std::{
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
    time::Duration,
};
use xbrl_rs::{TaxonomyLoader, TaxonomySet};
use zip::ZipArchive;

/// Returns the path to the application's taxonomy directory, which is located
/// in the user's data directory.
pub fn taxonomy_dir() -> Result<PathBuf, anyhow::Error> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("taxonomies"))
        .context("Could not determine data directory")
}

/// Derives the on-disk relative paths (under `taxonomy_dir`) for a set of
/// `link:schemaRef` URLs, used to check whether taxonomy files are already
/// cached.
pub fn schema_ref_paths(schema_refs: &[String]) -> Vec<String> {
    schema_refs
        .iter()
        .filter_map(|url| url.split("/taxonomies/").nth(1).map(str::to_string))
        .collect()
}

/// Downloads (if not already cached) and loads the taxonomy for the given
/// type and version, into `taxonomy_dir`. This is the only place a network
/// call is made on behalf of a taxonomy; callers that must stay offline
/// should use `load_taxonomies(..., allow_download: false)` instead and
/// surface a message pointing the user at this function (e.g. `taxel-cli`'s
/// `download` command).
pub fn download_taxonomy(
    taxonomy_type: &TaxonomyType,
    version: &str,
    taxonomy_dir: &Path,
) -> Result<TaxonomySet, anyhow::Error> {
    let date = TAXONOMY_VERSION_TO_DATE
        .get(version)
        .with_context(|| format!("No taxonomy date known for version {version}"))?;

    let schema_refs = taxonomy_type.schema_refs(date);
    let paths = schema_ref_paths(&schema_refs);
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    load_taxonomies(schema_refs, &path_refs, true, taxonomy_dir)?
        .context("Failed to load taxonomy after download")
}

/// Loads the taxonomies required for the given schema refs, from
/// `taxonomy_dir`. If they are missing and `allow_download` is false,
/// returns Ok(None) so the caller can ask for confirmation before
/// downloading.
pub fn load_taxonomies(
    schema_refs: Vec<String>,
    schema_ref_paths: &[&str],
    allow_download: bool,
    taxonomy_dir: &Path,
) -> Result<Option<TaxonomySet>, anyhow::Error> {
    let loader = TaxonomyLoader::new()?;

    let taxonomies_missing = schema_ref_paths
        .iter()
        .any(|path| !taxonomy_dir.join(path).exists());

    if taxonomies_missing {
        if !allow_download {
            return Ok(None);
        }

        if !taxonomy_dir.exists() {
            fs::create_dir_all(taxonomy_dir).with_context(|| {
                format!(
                    "Failed to create taxonomy directory: {}",
                    taxonomy_dir.display()
                )
            })?;
        }

        let taxonomies_missing = match loader.download_all(&schema_refs, taxonomy_dir) {
            Ok(result) => !result.failed.is_empty(),
            Err(err) => {
                debug!("Primary taxonomy download returned error: {err}");
                true
            }
        };

        if taxonomies_missing {
            debug!("Taxonomy files still missing after primary download, trying zip fallback");

            let version = taxonomy_version_from_schema_refs(&schema_refs)
                .context("Cannot determine taxonomy version for zip fallback")?;
            let date = TAXONOMY_VERSION_TO_DATE
                .get(version)
                .with_context(|| format!("No date known for taxonomy version {version}"))?;

            download_taxonomy_zip(version, date, taxonomy_dir)
                .with_context(|| format!("Zip fallback failed for taxonomy v{version}"))?;
        }
    }

    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_dir.to_path_buf())?;

    Ok(Some(taxonomy))
}

/// Downloads the bundled taxonomy zip for `version`/`date` from
/// `https://www.xbrl.de/german-gaap-taxonomy-v{version}-{date}.zip` and
/// extracts all entries under the `xbrl/` subfolder into `taxonomy_dir`.
fn download_taxonomy_zip(
    version: &str,
    date: &str,
    taxonomy_dir: &Path,
) -> Result<(), anyhow::Error> {
    let url = format!("https://www.xbrl.de/german-gaap-taxonomy-v{version}-{date}.zip");
    debug!("Downloading taxonomy zip from {url}");

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to GET {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Taxonomy zip download returned HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .context("Failed to read taxonomy zip body")?;
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("Failed to open taxonomy zip")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_owned();

        // The zip has a top-level folder:
        // german-gaap-taxonomy-v6.9-2025-04-01/xbrl/de-gcd-2025-04-01/... Find
        // "/xbrl/" anywhere in the path and take the remainder.
        let Some(xbrl_pos) = entry_name.find("/xbrl/") else {
            continue;
        };
        let relative = &entry_name[xbrl_pos + "/xbrl/".len()..];

        if relative.is_empty() {
            continue;
        }

        let dest = taxonomy_dir.join(relative);

        if !dest.starts_with(taxonomy_dir) {
            continue;
        }

        if entry.is_dir() {
            fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&dest)
                .with_context(|| format!("Failed to create {}", dest.display()))?;

            io::copy(&mut entry, &mut outfile)?;
        }
    }

    Ok(())
}
