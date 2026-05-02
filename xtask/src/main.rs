use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};
use zip::{write::SimpleFileOptions, ZipWriter};

const PACKAGES_DIR: &str = "extensions/registry/packages";
const BUNDLED_PLUGINS_DIR: &str = "extensions/bundled/plugins";
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
        "prepare-bundled-plugins" => {
            let options = PrepareBundledPluginsOptions::parse(rest)?;
            let plugins = prepare_bundled_plugins(&options)?;
            for plugin in plugins {
                println!("prepared={} wasm={}", plugin.id, plugin.wasm_path.display());
            }
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

#[derive(Debug)]
struct PrepareBundledPluginsOptions {
    plugins_dir: PathBuf,
}

impl PrepareBundledPluginsOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut plugins_dir = PathBuf::from(BUNDLED_PLUGINS_DIR);

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--plugins-dir" => {
                    plugins_dir = PathBuf::from(next_value(&mut iter, "--plugins-dir")?)
                }
                _ => {
                    return Err(usage(format!(
                        "unknown prepare-bundled-plugins option: {arg}"
                    )))
                }
            }
        }

        Ok(Self { plugins_dir })
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
        "{}\n\nusage:\n  cargo run -p xtask -- detect-changed [--base <sha> --head <sha>] [--extension-id <id>] [--force-all]\n  cargo run -p xtask -- package-extension --package-root <path> [--update-catalog]\n  cargo run -p xtask -- prepare-bundled-plugins [--plugins-dir <path>]",
        message.as_ref()
    )
}

#[derive(Debug, PartialEq, Eq)]
struct RustBundledPlugin {
    id: String,
    root: PathBuf,
    package_name: String,
    wasm_path: PathBuf,
}

impl RustBundledPlugin {
    fn release_wasm_path(&self) -> PathBuf {
        self.root
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("{}.wasm", self.package_name.replace('-', "_")))
    }
}

fn prepare_bundled_plugins(
    options: &PrepareBundledPluginsOptions,
) -> Result<Vec<RustBundledPlugin>, String> {
    let plugins = discover_rust_bundled_plugins(&options.plugins_dir)?;
    for plugin in &plugins {
        build_rust_bundled_plugin(plugin)?;
        let source = plugin.release_wasm_path();
        if !source.is_file() {
            return Err(format!(
                "missing built wasm for {}: {}",
                plugin.id,
                source.display()
            ));
        }
        fs::copy(&source, &plugin.wasm_path).map_err(|err| {
            format!(
                "failed to copy {} to {}: {err}",
                source.display(),
                plugin.wasm_path.display()
            )
        })?;
    }

    Ok(plugins)
}

fn discover_rust_bundled_plugins(plugins_dir: &Path) -> Result<Vec<RustBundledPlugin>, String> {
    let mut plugins = Vec::new();
    for entry in read_dir_sorted(plugins_dir)? {
        if !entry.file_type().map_err(|err| err.to_string())?.is_dir() {
            continue;
        }

        let root = entry.path();
        let plugin_toml_path = root.join("plugin.toml");
        if !plugin_toml_path.is_file() {
            continue;
        }

        let manifest: BundledPluginManifest = toml::from_str(
            &fs::read_to_string(&plugin_toml_path)
                .map_err(|err| format!("failed to read {}: {err}", plugin_toml_path.display()))?,
        )
        .map_err(|err| format!("failed to parse {}: {err}", plugin_toml_path.display()))?;

        let cargo_toml_path = root.join("Cargo.toml");
        if !cargo_toml_path.is_file() {
            continue;
        }

        let cargo_manifest: CargoManifest = toml::from_str(
            &fs::read_to_string(&cargo_toml_path)
                .map_err(|err| format!("failed to read {}: {err}", cargo_toml_path.display()))?,
        )
        .map_err(|err| format!("failed to parse {}: {err}", cargo_toml_path.display()))?;

        plugins.push(RustBundledPlugin {
            id: manifest.id,
            root: root.clone(),
            package_name: cargo_manifest.package.name,
            wasm_path: root.join(manifest.wasm),
        });
    }

    Ok(plugins)
}

fn build_rust_bundled_plugin(plugin: &RustBundledPlugin) -> Result<(), String> {
    let manifest_path = plugin.root.join("Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            manifest_path
                .to_str()
                .ok_or_else(|| format!("invalid path: {}", manifest_path.display()))?,
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .output()
        .map_err(|err| format!("failed to run cargo build for {}: {err}", plugin.id))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("failed to build {}:\n{}", plugin.id, stderr.trim()));
    }

    Ok(())
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
        if components.len() < packages_components.len() + 2 {
            continue;
        }
        if components[..packages_components.len()] != packages_components {
            continue;
        }

        let extension_id = &components[packages_components.len()];
        let relative = &components[packages_components.len() + 1..];
        if relative.len() == 1 && matches!(relative[0].as_str(), "manifest.toml" | "actions.toml") {
            if let Ok(platform_roots) = package_roots_for_extension(packages_dir, extension_id) {
                roots.extend(platform_roots);
            }
            continue;
        }

        let platform = &relative[0];
        if matches!(platform.as_str(), "manifest.toml" | "actions.toml") {
            continue;
        }
        roots.insert(packages_dir.join(extension_id).join(platform));
    }

    roots.into_iter().collect()
}

fn normalized_components(path: &Path) -> Vec<String> {
    path.components()
        .flat_map(|component| match component {
            Component::Normal(value) => value
                .to_string_lossy()
                .replace('\\', "/")
                .split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
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
    let extension_root = package_root
        .parent()
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?;
    let platform = package_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?;
    let manifest_path = extension_root.join("manifest.toml");
    let actions_path = extension_root.join("actions.toml");
    if !manifest_path.is_file() {
        return Err(format!("missing manifest: {}", manifest_path.display()));
    }
    if !actions_path.is_file() {
        return Err(format!(
            "missing actions metadata: {}",
            actions_path.display()
        ));
    }

    let manifest: PackageManifest = toml::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|err| format!("failed to read {}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", manifest_path.display()))?;

    validate_package_root(package_root, &manifest, platform)?;

    fs::create_dir_all(&options.output_dir).map_err(|err| err.to_string())?;
    let artifact_name = format!("{}-{}-{}.gpext", manifest.id, manifest.version, platform);
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
            platform,
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

fn validate_package_root(
    package_root: &Path,
    manifest: &PackageManifest,
    platform: &str,
) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported package schema version: {}",
            manifest.schema_version
        ));
    }

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
    if manifest.kind != "static" {
        return Err(format!("unsupported package kind: {}", manifest.kind));
    }
    if manifest
        .description
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err("manifest description must not be empty".to_string());
    }
    if manifest
        .repository
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err("manifest repository must not be empty".to_string());
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
    if static_config.version != 3 {
        return Err(format!(
            "unsupported static implementation version: {}",
            static_config.version
        ));
    }
    if static_config.platform != platform {
        return Err(format!(
            "static platform {} does not match folder platform {}",
            static_config.platform, platform
        ));
    }
    if static_config.process_name.trim().is_empty() {
        return Err("static process_name must not be empty".to_string());
    }

    let actions_path = package_root
        .parent()
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?
        .join("actions.toml");
    let actions_config: ActionsConfig = toml::from_str(
        &fs::read_to_string(&actions_path)
            .map_err(|err| format!("failed to read {}: {err}", actions_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", actions_path.display()))?;
    validate_action_mapping(&actions_config, &static_config)?;

    Ok(())
}

fn validate_action_mapping(
    actions_config: &ActionsConfig,
    static_config: &StaticConfig,
) -> Result<(), String> {
    if actions_config.schema_version != 1 {
        return Err(format!(
            "unsupported actions schema version: {}",
            actions_config.schema_version
        ));
    }
    if actions_config.actions.is_empty() {
        return Err("actions.toml must define at least one action".to_string());
    }

    for (action_id, metadata) in &actions_config.actions {
        if metadata.name.trim().is_empty() {
            return Err(format!("action '{action_id}' name must not be empty"));
        }
        let Some(implementation) = static_config.actions.get(action_id) else {
            return Err(format!(
                "action '{action_id}' has no platform implementation or pass entry"
            ));
        };
        validate_static_action(action_id, implementation)?;
    }

    for (action_id, implementation) in &static_config.actions {
        if !actions_config.actions.contains_key(action_id) {
            return Err(format!(
                "platform implementation references unknown action '{action_id}'"
            ));
        }
        validate_static_action(action_id, implementation)?;
    }

    Ok(())
}

fn validate_static_action(action_id: &str, action: &StaticAction) -> Result<(), String> {
    if action.cmd.is_some() && action.implementation.is_some() {
        return Err(format!(
            "action '{action_id}' must not set both cmd and implementation"
        ));
    }
    if action.implementation == Some(StaticImplementation::Pass) {
        return Ok(());
    }
    if action.implementation.is_some() {
        return Err(format!(
            "action '{action_id}' has unsupported implementation marker"
        ));
    }
    if action.cmd.is_none() {
        return Err(format!(
            "action '{action_id}' must set cmd or implementation = \"pass\""
        ));
    }
    Ok(())
}

fn create_package_archive(package_root: &Path, artifact_path: &Path) -> Result<(), String> {
    let file = fs::File::create(artifact_path)
        .map_err(|err| format!("failed to create {}: {err}", artifact_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for (source_path, archive_path) in package_files(package_root)? {
        zip.start_file(archive_path, options)
            .map_err(|err| err.to_string())?;
        let bytes = fs::read(&source_path)
            .map_err(|err| format!("failed to read {}: {err}", source_path.display()))?;
        zip.write_all(&bytes).map_err(|err| err.to_string())?;
    }

    zip.finish().map_err(|err| err.to_string())?;
    Ok(())
}

fn package_files(package_root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let extension_root = package_root
        .parent()
        .ok_or_else(|| format!("invalid package root: {}", package_root.display()))?;
    let manifest: PackageManifest = toml::from_str(
        &fs::read_to_string(extension_root.join("manifest.toml"))
            .map_err(|err| format!("failed to read manifest: {err}"))?,
    )
    .map_err(|err| format!("failed to parse manifest: {err}"))?;
    let static_path = package_root
        .join("static")
        .join(format!("{}.toml", manifest.id));
    let mut files = vec![
        (
            extension_root.join("manifest.toml"),
            "manifest.toml".to_string(),
        ),
        (
            extension_root.join("actions.toml"),
            "actions.toml".to_string(),
        ),
        (static_path, format!("static/{}.toml", manifest.id)),
    ];
    files.sort_by(|left, right| left.1.cmp(&right.1));
    Ok(files)
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
    platform: &str,
    package_url: &str,
    sha256: &str,
    size_bytes: u64,
) -> Result<(), String> {
    let mut catalog: Catalog = serde_json::from_slice(
        &fs::read(catalog_path)
            .map_err(|err| format!("failed to read {}: {err}", catalog_path.display()))?,
    )
    .map_err(|err| format!("failed to parse {}: {err}", catalog_path.display()))?;

    let entry_index = catalog
        .entries
        .iter()
        .position(|entry| entry.id == manifest.id && entry.platform == platform);
    if entry_index.is_none() {
        catalog.entries.push(CatalogEntry {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            platform: platform.to_string(),
            kind: manifest.kind.clone(),
            package_url: package_url.to_string(),
            package_sha256: sha256.to_string(),
            size_bytes: Some(size_bytes),
            publisher: manifest.publisher.clone(),
            description: manifest.description.clone(),
            license: manifest.license.clone(),
            homepage: manifest.homepage.clone(),
            repository: manifest.repository.clone(),
            keywords: manifest.keywords.clone(),
            min_app_version: manifest.min_app_version.clone(),
        });
    }

    let entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id == manifest.id && entry.platform == platform)
        .expect("catalog entry should exist");

    entry.name = manifest.name.clone();
    entry.version = manifest.version.clone();
    entry.kind = manifest.kind.clone();
    entry.package_url = package_url.to_string();
    entry.package_sha256 = sha256.to_string();
    entry.size_bytes = Some(size_bytes);
    if let Some(publisher) = &manifest.publisher {
        entry.publisher = Some(publisher.clone());
    }
    entry.description = manifest.description.clone();
    entry.license = manifest.license.clone();
    entry.homepage = manifest.homepage.clone();
    entry.repository = manifest.repository.clone();
    entry.keywords = manifest.keywords.clone();
    entry.min_app_version = manifest.min_app_version.clone();
    catalog.entries.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.platform.cmp(&right.platform))
    });

    let content = serde_json::to_string_pretty(&catalog)
        .map_err(|err| format!("failed to serialize catalog: {err}"))?;
    fs::write(catalog_path, format!("{content}\n"))
        .map_err(|err| format!("failed to write {}: {err}", catalog_path.display()))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PackageManifest {
    schema_version: u32,
    id: String,
    name: String,
    version: String,
    kind: String,
    publisher: Option<String>,
    description: Option<String>,
    license: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    min_app_version: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BundledPluginManifest {
    id: String,
    wasm: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: CargoPackage,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaticConfig {
    version: u32,
    platform: String,
    process_name: String,
    actions: BTreeMap<String, StaticAction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaticAction {
    cmd: Option<toml::Value>,
    implementation: Option<StaticImplementation>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum StaticImplementation {
    Pass,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(deny_unknown_fields)]
struct ActionsConfig {
    schema_version: u32,
    app: Option<toml::Value>,
    actions: BTreeMap<String, ActionMetadata>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(deny_unknown_fields)]
struct ActionMetadata {
    name: String,
    focus_state: Option<String>,
    when: Option<toml::Value>,
    #[serde(alias = "action_priority")]
    priority: Option<String>,
    tags: Option<Vec<String>>,
    favorite: Option<bool>,
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
        let root = tempfile_dir();
        let packages_dir = root.join("extensions").join("registry").join("packages");
        fs::create_dir_all(packages_dir.join("file_explorer").join("windows")).unwrap();
        let roots = changed_package_roots(
            &[packages_dir.join("file_explorer").join("manifest.toml")],
            &packages_dir,
        );

        assert_eq!(
            roots,
            vec![packages_dir.join("file_explorer").join("windows")]
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
        let root = tempfile_dir();
        let packages_dir = root.join("extensions").join("registry").join("packages");
        fs::create_dir_all(packages_dir.join("chrome").join("windows")).unwrap();
        fs::create_dir_all(packages_dir.join("powerpoint").join("windows")).unwrap();
        let roots = changed_package_roots(
            &[
                packages_dir.join("powerpoint").join("manifest.toml"),
                packages_dir
                    .join("chrome")
                    .join("windows")
                    .join("static")
                    .join("chrome.toml"),
                packages_dir.join("chrome").join("actions.toml"),
            ],
            &packages_dir,
        );

        assert_eq!(
            roots,
            vec![
                packages_dir.join("chrome").join("windows"),
                packages_dir.join("powerpoint").join("windows"),
            ]
        );
    }

    #[test]
    fn manifest_folder_validation_accepts_matching_static_package() {
        let root = tempfile_dir();
        let package_root = root.join("extensions/registry/packages/chrome/windows");
        fs::create_dir_all(package_root.join("static")).unwrap();
        fs::write(
            package_root.parent().unwrap().join("manifest.toml"),
            r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "0.1.0"
kind = "static"
description = "Chrome."
repository = "https://github.com/Greg-Lim/omni-palette-desktop"
"#,
        )
        .unwrap();
        fs::write(
            package_root.parent().unwrap().join("actions.toml"),
            r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
        )
        .unwrap();
        fs::write(
            package_root.join("static/chrome.toml"),
            r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
        )
        .unwrap();

        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(package_root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        assert!(validate_package_root(&package_root, &manifest, "windows").is_ok());
    }

    #[test]
    fn manifest_id_mismatch_fails() {
        let root = root_with_package("chrome", "windows", "wrong", "windows", true);
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("does not match folder id"));
    }

    #[test]
    fn static_platform_mismatch_fails() {
        let root = root_with_package("chrome", "windows", "chrome", "macos", true);
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("does not match folder platform"));
    }

    #[test]
    fn missing_static_file_fails() {
        let root = root_with_package("chrome", "windows", "chrome", "windows", false);
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("missing static extension file"));
    }

    #[test]
    fn pass_static_action_validates_without_command() {
        let root = root_with_split_package(
            "chrome",
            r#"schema_version = 1

[actions.new_tab]
name = "New tab"

[actions.macos_only]
name = "macOS only"
"#,
            r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }

[actions.macos_only]
implementation = "pass"
"#,
        );
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        assert!(validate_package_root(&root, &manifest, "windows").is_ok());
    }

    #[test]
    fn unknown_static_action_fails_validation() {
        let root = root_with_split_package(
            "chrome",
            r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
            r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }

[actions.unknown]
cmd = { mods = ["ctrl"], key = "KeyU" }
"#,
        );
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("unknown action"));
    }

    #[test]
    fn missing_static_action_mapping_fails_validation() {
        let root = root_with_split_package(
            "chrome",
            r#"schema_version = 1

[actions.new_tab]
name = "New tab"

[actions.close_tab]
name = "Close tab"
"#,
            r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
        );
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("no platform implementation"));
    }

    #[test]
    fn old_metadata_in_static_action_fails_validation() {
        let root = root_with_split_package(
            "chrome",
            r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
            r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
name = "New tab"
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
        );
        let manifest: PackageManifest = toml::from_str(
            &fs::read_to_string(root.parent().unwrap().join("manifest.toml")).unwrap(),
        )
        .unwrap();

        let err = validate_package_root(&root, &manifest, "windows").unwrap_err();

        assert!(err.contains("unknown field"));
    }

    #[test]
    fn update_catalog_entry_creates_missing_entry_from_manifest_metadata() {
        let root = tempfile_dir();
        let catalog_path = root.join("catalog.v1.json");
        fs::write(
            &catalog_path,
            r#"{"schema_version":1,"generated_at":null,"expires_at_unix":null,"entries":[]}"#,
        )
        .unwrap();
        let manifest = PackageManifest {
            schema_version: 1,
            id: "windows".to_string(),
            name: "Windows".to_string(),
            version: "0.1.0".to_string(),
            kind: "static".to_string(),
            publisher: Some("Greg-Lim".to_string()),
            description: Some("Windows system keyboard shortcut command pack.".to_string()),
            license: None,
            homepage: None,
            repository: Some("https://github.com/Greg-Lim/omni-palette-desktop".to_string()),
            keywords: vec!["windows".to_string(), "shortcuts".to_string()],
            min_app_version: None,
            permissions: Vec::new(),
        };

        update_catalog_entry(
            &catalog_path,
            &manifest,
            "windows",
            "https://github.com/Greg-Lim/omni-palette-desktop/releases/download/windows-v0.1.0/windows-0.1.0-windows.gpext",
            &"a".repeat(64),
            123,
        )
        .expect("catalog entry should upsert");

        let catalog: Catalog =
            serde_json::from_slice(&fs::read(catalog_path).expect("catalog should read"))
                .expect("catalog should parse");

        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.entries[0].id, "windows");
        assert_eq!(catalog.entries[0].platform, "windows");
        assert_eq!(catalog.entries[0].keywords, vec!["windows", "shortcuts"]);
        assert_eq!(catalog.entries[0].size_bytes, Some(123));
    }

    #[test]
    fn discovers_only_rust_bundled_plugins() {
        let root = tempfile_dir();
        let plugins_dir = root.join("extensions/bundled/plugins");
        let rust_plugin = plugins_dir.join("ahk_agent");
        let wat_plugin = plugins_dir.join("auto_typer");
        fs::create_dir_all(&rust_plugin).unwrap();
        fs::create_dir_all(&wat_plugin).unwrap();
        fs::write(
            rust_plugin.join("plugin.toml"),
            r#"id = "ahk_agent"
name = "AHK"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wasm"
"#,
        )
        .unwrap();
        fs::write(
            rust_plugin.join("Cargo.toml"),
            r#"[package]
name = "ahk_agent_wasm"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(
            wat_plugin.join("plugin.toml"),
            r#"id = "auto_typer"
name = "Auto Typer"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
"#,
        )
        .unwrap();

        let plugins = discover_rust_bundled_plugins(&plugins_dir).unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "ahk_agent");
        assert_eq!(plugins[0].package_name, "ahk_agent_wasm");
        assert_eq!(plugins[0].wasm_path, rust_plugin.join("plugin.wasm"));
    }

    #[test]
    fn rust_bundled_plugin_output_path_uses_manifest_package_name() {
        let plugin = RustBundledPlugin {
            id: "ahk_agent".to_string(),
            root: PathBuf::from("extensions/bundled/plugins/ahk_agent"),
            package_name: "ahk_agent_wasm".to_string(),
            wasm_path: PathBuf::from("extensions/bundled/plugins/ahk_agent/plugin.wasm"),
        };

        assert_eq!(
            plugin.release_wasm_path(),
            PathBuf::from(
                "extensions/bundled/plugins/ahk_agent/target/wasm32-unknown-unknown/release/ahk_agent_wasm.wasm"
            )
        );
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
            package_root.parent().unwrap().join("manifest.toml"),
            format!(
                r#"schema_version = 1
id = "{manifest_id}"
name = "Test"
version = "0.1.0"
kind = "static"
description = "Test package."
repository = "https://github.com/Greg-Lim/omni-palette-desktop"
"#
            ),
        )
        .unwrap();
        fs::write(
            package_root.parent().unwrap().join("actions.toml"),
            r#"schema_version = 1

[actions.test]
name = "Test"
"#,
        )
        .unwrap();
        if write_static {
            fs::write(
                package_root
                    .join("static")
                    .join(format!("{manifest_id}.toml")),
                format!(
                    r#"version = 3
platform = "{manifest_platform}"
process_name = "test.exe"

[actions.test]
cmd = {{ mods = ["ctrl"], key = "KeyT" }}
"#
                ),
            )
            .unwrap();
        }

        package_root
    }

    fn root_with_split_package(id: &str, actions: &str, static_impl: &str) -> PathBuf {
        let root = tempfile_dir();
        let package_root = root
            .join("extensions/registry/packages")
            .join(id)
            .join("windows");
        fs::create_dir_all(package_root.join("static")).unwrap();
        fs::write(
            package_root.parent().unwrap().join("manifest.toml"),
            format!(
                r#"schema_version = 1
id = "{id}"
name = "Test"
version = "0.1.0"
kind = "static"
description = "Test package."
repository = "https://github.com/Greg-Lim/omni-palette-desktop"
"#
            ),
        )
        .unwrap();
        fs::write(package_root.parent().unwrap().join("actions.toml"), actions).unwrap();
        fs::write(
            package_root.join("static").join(format!("{id}.toml")),
            static_impl,
        )
        .unwrap();
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
