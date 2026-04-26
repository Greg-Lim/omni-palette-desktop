use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};
use zip::{write::SimpleFileOptions, ZipWriter};

const PACKAGES_DIR: &str = "extensions/registry/packages";
const CATALOG_PATH: &str = "extensions/registry/catalog.v1.json";
const OUTPUT_DIR: &str = "target/extensions";
const DEFAULT_REPOSITORY_URL: &str = "https://github.com/Greg-Lim/omni-palette-desktop";

fn main() {
    if let Err(err) = run(env::args().skip(1).collect()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or_else(|| usage("missing command"))?;

    match command.as_str() {
        "detect-changed" => {
            let options = DetectOptions::parse(rest)?;
            let roots = detect_changed_package_roots(&options)?;
            for root in roots {
                println!("{}", root.display());
            }
            Ok(())
        }
        "package-extension" => {
            let options = PackageOptions::parse(rest)?;
            let result = package_extension(&options)?;
            println!("artifact={}", result.artifact_path.display());
            println!("sha256={}", result.sha256);
            println!("size_bytes={}", result.size_bytes);
            println!("package_url={}", result.package_url);
            Ok(())
        }
        _ => Err(usage(format!("unknown command: {command}"))),
    }
}

#[derive(Debug)]
struct DetectOptions {
    base: Option<String>,
    head: Option<String>,
    extension_id: Option<String>,
    force_all: bool,
    packages_dir: PathBuf,
}

impl DetectOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            base: None,
            head: None,
            extension_id: None,
            force_all: false,
            packages_dir: PathBuf::from(PACKAGES_DIR),
        };

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--base" => options.base = Some(next_value(&mut iter, "--base")?),
                "--head" => options.head = Some(next_value(&mut iter, "--head")?),
                "--extension-id" => {
                    options.extension_id = Some(next_value(&mut iter, "--extension-id")?)
                }
                "--force-all" => options.force_all = true,
                "--packages-dir" => {
                    options.packages_dir = PathBuf::from(next_value(&mut iter, "--packages-dir")?)
                }
                _ => return Err(usage(format!("unknown detect-changed option: {arg}"))),
            }
        }

        Ok(options)
    }
}

#[derive(Debug)]
struct PackageOptions {
    package_root: PathBuf,
    catalog_path: PathBuf,
    output_dir: PathBuf,
    update_catalog: bool,
    repository_url: String,
}

impl PackageOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut package_root = None;
        let mut catalog_path = PathBuf::from(CATALOG_PATH);
        let mut output_dir = PathBuf::from(OUTPUT_DIR);
        let mut update_catalog = false;
        let mut repository_url = DEFAULT_REPOSITORY_URL.to_string();

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--package-root" => {
                    package_root = Some(PathBuf::from(next_value(&mut iter, "--package-root")?))
                }
                "--catalog" => catalog_path = PathBuf::from(next_value(&mut iter, "--catalog")?),
                "--output-dir" => {
                    output_dir = PathBuf::from(next_value(&mut iter, "--output-dir")?)
                }
                "--update-catalog" => update_catalog = true,
                "--repository-url" => {
                    repository_url = next_value(&mut iter, "--repository-url")?;
                }
                _ => return Err(usage(format!("unknown package-extension option: {arg}"))),
            }
        }

        Ok(Self {
            package_root: package_root.ok_or_else(|| usage("missing --package-root"))?,
            catalog_path,
            output_dir,
            update_catalog,
            repository_url,
        })
    }
}

fn next_value<'a>(
    iter: &mut impl Iterator<Item = &'a String>,
    option: &str,
) -> Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| usage(format!("{option} requires a value")))
}

fn usage(message: impl AsRef<str>) -> String {
    format!(
        "{}\n\nusage:\n  cargo run -p xtask -- detect-changed [--base <sha> --head <sha>] [--extension-id <id>] [--force-all]\n  cargo run -p xtask -- package-extension --package-root <path> [--update-catalog]",
        message.as_ref()
    )
}

fn detect_changed_package_roots(options: &DetectOptions) -> Result<Vec<PathBuf>, String> {
    if options.force_all {
        return all_package_roots(&options.packages_dir, options.extension_id.as_deref());
    }

    if let Some(extension_id) = &options.extension_id {
        return package_roots_for_extension(&options.packages_dir, extension_id);
    }

    let changed_paths = match (&options.base, &options.head) {
        (Some(base), Some(head)) => git_changed_paths(base, head)?,
        _ => {
            return Err(
                "no package roots changed; pass --base/--head, --extension-id, or --force-all"
                    .to_string(),
            )
        }
    };

    let roots = changed_package_roots(&changed_paths, &options.packages_dir);
    if roots.is_empty() {
        return Err("no package roots changed".to_string());
    }

    Ok(roots)
}

fn all_package_roots(
    packages_dir: &Path,
    extension_filter: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    for extension in read_dir_sorted(packages_dir)? {
        if !extension
            .file_type()
            .map_err(|err| err.to_string())?
            .is_dir()
        {
            continue;
        }

        let extension_id = extension.file_name().to_string_lossy().to_string();
        if extension_filter.is_some_and(|filter| filter != extension_id) {
            continue;
        }

        for platform in read_dir_sorted(&extension.path())? {
            if platform
                .file_type()
                .map_err(|err| err.to_string())?
                .is_dir()
            {
                roots.push(platform.path());
            }
        }
    }

    if roots.is_empty() {
        Err("no package roots found".to_string())
    } else {
        Ok(roots)
    }
}

fn package_roots_for_extension(
    packages_dir: &Path,
    extension_id: &str,
) -> Result<Vec<PathBuf>, String> {
    all_package_roots(packages_dir, Some(extension_id))
}

fn read_dir_sorted(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    let mut entries = fs::read_dir(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|err| err.to_string())?;
    entries.sort_by_key(|entry| entry.path());
    Ok(entries)
}

fn git_changed_paths(base: &str, head: &str) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", base, head])
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

fn changed_package_roots(changed_paths: &[PathBuf], packages_dir: &Path) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    let packages_components = normalized_components(packages_dir);

    for path in changed_paths {
        let components = normalized_components(path);
        if components.len() < packages_components.len() + 3 {
            continue;
        }
        if components[..packages_components.len()] != packages_components {
            continue;
        }

        let extension_id = &components[packages_components.len()];
        let platform = &components[packages_components.len() + 1];
        roots.insert(packages_dir.join(extension_id).join(platform));
    }

    roots.into_iter().collect()
}

fn normalized_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().replace('\\', "/")),
            _ => None,
        })
        .collect()
}

#[derive(Debug)]
struct PackageResult {
    artifact_path: PathBuf,
    sha256: String,
    size_bytes: u64,
    package_url: String,
}

fn package_extension(options: &PackageOptions) -> Result<PackageResult, String> {
    let package_root = options.package_root.as_path();
    let manifest_path = package_root.join("manifest.toml");
    if !manifest_path.is_file() {
        return Err(format!("missing manifest: {}", manifest_path.display()));
    }

    let manifest: PackageManifest = toml::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|err| format!("failed to read {}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", manifest_path.display()))?;

    validate_package_root(package_root, &manifest)?;

    fs::create_dir_all(&options.output_dir).map_err(|err| err.to_string())?;
    let artifact_name = format!(
        "{}-{}-{}.gpext",
        manifest.id, manifest.version, manifest.platform
    );
    let artifact_path = options.output_dir.join(&artifact_name);
    if artifact_path.exists() {
        fs::remove_file(&artifact_path).map_err(|err| err.to_string())?;
    }
    create_package_archive(package_root, &artifact_path)?;

    let sha256 = sha256_file(&artifact_path)?;
    let size_bytes = fs::metadata(&artifact_path)
        .map_err(|err| err.to_string())?
        .len();
    let package_url = format!(
        "{}/releases/download/{}-v{}/{}",
        options.repository_url.trim_end_matches('/'),
        manifest.id,
        manifest.version,
        artifact_name
    );

    if options.update_catalog {
        update_catalog_entry(
            &options.catalog_path,
            &manifest,
            &package_url,
            &sha256,
            size_bytes,
        )?;
    }

    Ok(PackageResult {
        artifact_path,
        sha256,
        size_bytes,
        package_url,
    })
}

fn validate_package_root(package_root: &Path, manifest: &PackageManifest) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported package schema version: {}",
            manifest.schema_version
        ));
    }

    let platform_folder = package_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?;
    let id_folder = package_root
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?;

    if manifest.id != id_folder {
        return Err(format!(
            "manifest id {} does not match folder id {}",
            manifest.id, id_folder
        ));
    }
    if manifest.platform != platform_folder {
        return Err(format!(
            "manifest platform {} does not match folder platform {}",
            manifest.platform, platform_folder
        ));
    }
    if manifest.kind != "static" {
        return Err(format!("unsupported package kind: {}", manifest.kind));
    }

    let static_path = package_root
        .join("static")
        .join(format!("{}.toml", manifest.id));
    if !static_path.is_file() {
        return Err(format!(
            "missing static extension file: {}",
            static_path.display()
        ));
    }

    let static_config: StaticConfig = toml::from_str(
        &fs::read_to_string(&static_path)
            .map_err(|err| format!("failed to read {}: {err}", static_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", static_path.display()))?;
    if static_config.platform != manifest.platform {
        return Err(format!(
            "static platform {} does not match manifest platform {}",
            static_config.platform, manifest.platform
        ));
    }
    if static_config.app.id != manifest.id {
        return Err(format!(
            "static app id {} does not match manifest id {}",
            static_config.app.id, manifest.id
        ));
    }

    Ok(())
}

fn create_package_archive(package_root: &Path, artifact_path: &Path) -> Result<(), String> {
    let file = fs::File::create(artifact_path)
        .map_err(|err| format!("failed to create {}: {err}", artifact_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for source_path in package_files(package_root)? {
        let archive_path = source_path
            .strip_prefix(package_root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(archive_path, options)
            .map_err(|err| err.to_string())?;
        let bytes = fs::read(&source_path)
            .map_err(|err| format!("failed to read {}: {err}", source_path.display()))?;
        zip.write_all(&bytes).map_err(|err| err.to_string())?;
    }

    zip.finish().map_err(|err| err.to_string())?;
    Ok(())
}

fn package_files(package_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files(package_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in read_dir_sorted(path)? {
        let entry_path = entry.path();
        if entry.file_type().map_err(|err| err.to_string())?.is_dir() {
            collect_files(&entry_path, files)?;
        } else {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn update_catalog_entry(
    catalog_path: &Path,
    manifest: &PackageManifest,
    package_url: &str,
    sha256: &str,
    size_bytes: u64,
) -> Result<(), String> {
    let mut catalog: Catalog = serde_json::from_slice(
        &fs::read(catalog_path)
            .map_err(|err| format!("failed to read {}: {err}", catalog_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", catalog_path.display()))?;

    let entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id == manifest.id && entry.platform == manifest.platform)
        .ok_or_else(|| {
            format!(
                "catalog entry not found for {} {}",
                manifest.id, manifest.platform
            )
        })?;

    entry.name = manifest.name.clone();
    entry.version = manifest.version.clone();
    entry.kind = manifest.kind.clone();
    entry.package_url = package_url.to_string();
    entry.package_sha256 = sha256.to_string();
    entry.size_bytes = Some(size_bytes);
    if let Some(publisher) = &manifest.publisher {
        entry.publisher = Some(publisher.clone());
    }

    let content = serde_json::to_string_pretty(&catalog)
        .map_err(|err| format!("failed to serialize catalog: {err}"))?;
    fs::write(catalog_path, format!("{content}\n"))
        .map_err(|err| format!("failed to write {}: {err}", catalog_path.display()))
}

#[derive(Debug, Deserialize)]
struct PackageManifest {
    schema_version: u32,
    id: String,
    name: String,
    platform: String,
    version: String,
    kind: String,
    publisher: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StaticConfig {
    platform: String,
    app: StaticApp,
}

#[derive(Debug, Deserialize)]
struct StaticApp {
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Catalog {
    schema_version: u32,
    generated_at: Option<String>,
    expires_at_unix: Option<u64>,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CatalogEntry {
    id: String,
    name: String,
    version: String,
    platform: String,
    kind: String,
    package_url: String,
    package_sha256: String,
    size_bytes: Option<u64>,
    publisher: Option<String>,
    description: Option<String>,
    license: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    keywords: Vec<String>,
    min_app_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_toml_change_maps_to_package_root() {
        let roots = changed_package_roots(
            &[PathBuf::from(
                "extensions/registry/packages/chrome/windows/static/chrome.toml",
            )],
            Path::new(PACKAGES_DIR),
        );

        assert_eq!(
            roots,
            vec![PathBuf::from("extensions/registry/packages/chrome/windows")]
        );
    }

    #[test]
    fn manifest_change_maps_to_package_root() {
        let roots = changed_package_roots(
            &[PathBuf::from(
                "extensions/registry/packages/file_explorer/windows/manifest.toml",
            )],
            Path::new(PACKAGES_DIR),
        );

        assert_eq!(
            roots,
            vec![PathBuf::from(
                "extensions/registry/packages/file_explorer/windows"
            )]
        );
    }

    #[test]
    fn catalog_only_change_does_not_trigger_deploy() {
        let roots = changed_package_roots(
            &[PathBuf::from("extensions/registry/catalog.v1.json")],
            Path::new(PACKAGES_DIR),
        );

        assert!(roots.is_empty());
    }

    #[test]
    fn multiple_changes_produce_distinct_sorted_package_roots() {
        let roots = changed_package_roots(
            &[
                PathBuf::from("extensions/registry/packages/powerpoint/windows/manifest.toml"),
                PathBuf::from("extensions/registry/packages/chrome/windows/static/chrome.toml"),
                PathBuf::from("extensions/registry/packages/chrome/windows/manifest.toml"),
            ],
            Path::new(PACKAGES_DIR),
        );

        assert_eq!(
            roots,
            vec![
                PathBuf::from("extensions/registry/packages/chrome/windows"),
                PathBuf::from("extensions/registry/packages/powerpoint/windows"),
            ]
        );
    }

    #[test]
    fn manifest_folder_validation_accepts_matching_static_package() {
        let root = tempfile_dir();
        let package_root = root.join("extensions/registry/packages/chrome/windows");
        fs::create_dir_all(package_root.join("static")).unwrap();
        fs::write(
            package_root.join("manifest.toml"),
            r#"schema_version = 1
id = "chrome"
name = "Chrome"
platform = "windows"
version = "0.1.0"
kind = "static"
"#,
        )
        .unwrap();
        fs::write(
            package_root.join("static/chrome.toml"),
            r#"version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"
"#,
        )
        .unwrap();

        let manifest: PackageManifest =
            toml::from_str(&fs::read_to_string(package_root.join("manifest.toml")).unwrap())
                .unwrap();

        assert!(validate_package_root(&package_root, &manifest).is_ok());
    }

    #[test]
    fn manifest_id_mismatch_fails() {
        let root = root_with_package("chrome", "windows", "wrong", "windows", true);
        let manifest: PackageManifest =
            toml::from_str(&fs::read_to_string(root.join("manifest.toml")).unwrap()).unwrap();

        let err = validate_package_root(&root, &manifest).unwrap_err();

        assert!(err.contains("does not match folder id"));
    }

    #[test]
    fn manifest_platform_mismatch_fails() {
        let root = root_with_package("chrome", "windows", "chrome", "macos", true);
        let manifest: PackageManifest =
            toml::from_str(&fs::read_to_string(root.join("manifest.toml")).unwrap()).unwrap();

        let err = validate_package_root(&root, &manifest).unwrap_err();

        assert!(err.contains("does not match folder platform"));
    }

    #[test]
    fn missing_static_file_fails() {
        let root = root_with_package("chrome", "windows", "chrome", "windows", false);
        let manifest: PackageManifest =
            toml::from_str(&fs::read_to_string(root.join("manifest.toml")).unwrap()).unwrap();

        let err = validate_package_root(&root, &manifest).unwrap_err();

        assert!(err.contains("missing static extension file"));
    }

    fn root_with_package(
        folder_id: &str,
        folder_platform: &str,
        manifest_id: &str,
        manifest_platform: &str,
        write_static: bool,
    ) -> PathBuf {
        let root = tempfile_dir();
        let package_root = root
            .join("extensions/registry/packages")
            .join(folder_id)
            .join(folder_platform);
        fs::create_dir_all(package_root.join("static")).unwrap();
        fs::write(
            package_root.join("manifest.toml"),
            format!(
                r#"schema_version = 1
id = "{manifest_id}"
name = "Test"
platform = "{manifest_platform}"
version = "0.1.0"
kind = "static"
"#
            ),
        )
        .unwrap();
        if write_static {
            fs::write(
                package_root
                    .join("static")
                    .join(format!("{manifest_id}.toml")),
                format!(
                    r#"version = 2
platform = "{manifest_platform}"

[app]
id = "{manifest_id}"
name = "Test"
"#
                ),
            )
            .unwrap();
        }

        package_root
    }

    fn tempfile_dir() -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "omni-palette-xtask-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
