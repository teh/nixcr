// Docker v2 registry that shells out to nix to
// build layers.
// Could be better:
// * non-unwrap error handling
// * layer packing
// * more consistent naming for the docker JSON structure]
//
// TODO
// * git repo integration (fetch before build)
// * private cache (minio?)
// * caching from key to layers
// * error handling when commands fail
#[macro_use]
extern crate log;
use actix_files::NamedFile;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpResponse, HttpServer};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec::Vec;

mod store;

#[derive(Debug)]
struct Config {
    /// Directory to store and serve blobs. Must exist.
    blob_root: std::path::PathBuf,
    /// Where to clone the git repos
    repo_root: std::path::PathBuf,
    repo_configs: HashMap<String, RepoConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DockerManifestV2Config {
    media_type: String,
    size: usize,
    digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DockerManifestV2 {
    schema_version: usize,
    media_type: String,
    config: DockerManifestV2Config,
    layers: Vec<LayerMeta>,
}

#[derive(Serialize, Debug)]
struct RootFS {
    #[serde(rename = "type")]
    type_: String,
    diff_ids: Vec<String>, // NB no camel case
}

#[serde(rename_all = "camelCase")]
#[derive(Serialize)]
struct LayerMeta {
    media_type: String,
    size: usize,    // size of layer.tar
    digest: String, // compressed
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RootFSContainer {
    architecture: String,
    created: String,
    os: String,
    rootfs: RootFS,
}

enum Error {
    CloneFailed,
    FetchFailed,
    BuildFailed,
    QueryFailed,
    ArchiveFailed,
    CommitNotFound { commit: String },
}

struct HashAndWrite {
    // implement Write trait but pipe output into hasher as well as file.
    tar: std::io::BufWriter<std::fs::File>,
    digest: Sha256,
    size: usize,
}

/// implement io::Write and hash at the same time as writing the tarball
impl HashAndWrite {
    fn new(path: &std::path::Path) -> HashAndWrite {
        HashAndWrite {
            // the tar writer seems somewht slow, and BufWriter
            // didn't make it faster.
            tar: std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
            digest: Sha256::new(),
            size: 0,
        }
    }

    fn get_digest(&mut self) -> String {
        format!("sha256:{}", self.digest.result_str())
    }

    fn get_size(&mut self) -> usize {
        self.size
    }
}

impl std::io::Write for HashAndWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.digest.input(buf);
        self.size += buf.len();
        self.tar.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.tar.flush()
    }
}

/// clones (or fetches if already cloned) a git repo.
fn clone_or_fetch_repo(git_dir: &std::path::Path, repo_config: &RepoConfig) -> Result<(), Error> {
    if git_dir.is_dir() {
        info!("fetching for {:?}", git_dir);
        let fetch = std::process::Command::new("git")
            .arg("--git-dir")
            .arg(git_dir)
            .arg("fetch")
            .env(
                "GIT_SSH_COMMAND",
                match &repo_config.deploy_key_path {
                    Some(path) => format!("ssh -i {}", path.display()),
                    None => "ssh".to_string(),
                },
            )
            .status()
            .unwrap();
        if fetch.success() {
            Ok(())
        } else {
            Err(Error::FetchFailed)
        }
    } else {
        // TODO make repo configurable + ssh key env
        info!("cloning {:?} to {:?}", repo_config.url, git_dir);
        let clone = std::process::Command::new("git")
            .arg("clone")
            .arg("--bare")
            .arg(&repo_config.url)
            .arg(git_dir)
            .status()
            .unwrap();
        if clone.success() {
            Ok(())
        } else {
            Err(Error::CloneFailed)
        }
    }
}

fn get_git_tarball(
    git_dir: &std::path::Path,
    config: &Config,
    commit: &str,
) -> Result<std::path::PathBuf, Error> {
    // TODO - cache tarballs
    let archive = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .arg("archive")
        .arg("--prefix")
        // Prefix will be stripped by nix-build. The "/" is important: don't remove
        .arg("x/")
        .arg(commit)
        .output()
        .unwrap();

    if !archive.status.success() {
        // TODO - parse archive.stdout to look for stuff like missing commits
        return Err(Error::ArchiveFailed);
    }
    let tar_bytes = archive.stdout;

    // TODO - blob_root is served via http so probably not the best place
    // to store the tarballs.
    // TODO - process::Command write straight to a file?
    let tar_path = config.blob_root.join(commit);
    std::fs::write(&tar_path, tar_bytes).unwrap();

    Ok(tar_path)
}

fn build_layers(
    config: &Config,
    lookup_key: &str,
    commit: &str,
    attr_path: &str,
) -> Result<Vec<LayerMeta>, Error> {
    info!("looking up {}, commit {}", lookup_key, commit);
    let repo_config = match config.repo_configs.get(lookup_key) {
        Some(repo_config) => repo_config,
        None => panic!("Unknown lookup key {}", lookup_key),
    };
    // I really have't gotten the abstraction right here given that I have to
    // pass around repo_config everywhere. Maybe better
    let git_dir = config.repo_root.join(repo_config.git_dir());
    clone_or_fetch_repo(&git_dir, &repo_config)?;
    let tar_path = get_git_tarball(&git_dir, &config, &commit)?;

    let build = std::process::Command::new("nix-build")
        .arg(format!("file:///{}", tar_path.display()))
        .arg("-A")
        .arg(attr_path)
        .status()
        .expect("build failed");
    if !build.success() {
        return Err(Error::BuildFailed);
    }

    // TODO - rename out result so we can query it?
    //     alternatively query output from _build.
    let query = std::process::Command::new("nix-store")
        .arg("-qR")
        .arg("result")
        .output()
        .expect("query failed");
    if !query.status.success() {
        return Err(Error::QueryFailed);
    }

    // Dumb even-sized layer chunker, I believe the query
    // returns in dependency order so this chunker will at least
    // pack some stuff together correctly.
    let mut all_paths = Vec::new();
    for x in query.stdout.split(|c| *c == 0x0au8) {
        if x.is_empty() {
            continue;
        };
        all_paths.push(std::path::Path::new(std::str::from_utf8(x).unwrap()));
    }
    let paths_per_layer = all_paths.len() / 100usize + 1;

    // TODO - parametrize build path (temp folder?)
    let base_path = std::path::Path::new("/tmp/nixcr");

    let mut layers = Vec::new();
    for chunk in all_paths.chunks(paths_per_layer) {
        // TODO replace build.tar witih some temp thing
        let temp_path = base_path.join("layer.tar");
        let haw = HashAndWrite::new(&temp_path);
        let mut archive_builder = tar::Builder::new(haw);
        // keep symlinks intact which is the behaviour we want in docker images
        archive_builder.follow_symlinks(false);

        for x in chunk {
            archive_builder
                .append_dir_all(x.strip_prefix("/").unwrap(), x)
                .unwrap();
        }
        let mut archive = archive_builder.into_inner().unwrap();

        // Move built key to its digest (which we need to calculate anyway due
        // because it goes into the layer meta)
        layers.push(LayerMeta {
            media_type: "application/vnd.docker.image.rootfs.diff.tar.gzip".to_string(),
            digest: archive.get_digest(),
            size: archive.get_size(),
        });

        std::fs::rename(temp_path, config.blob_root.join(archive.get_digest()));
    }
    Ok(layers)
}

type HandlerConfig = web::Data<std::sync::Arc<Config>>;

fn blobs(
    config: HandlerConfig,
    info: web::Path<(String, String, String)>,
) -> actix_web::Result<NamedFile> {
    let blob_path = config.blob_root.join(info.2.clone());
    if !blob_path.is_file() {
        Err(actix_web::error::ErrorNotFound(""))
    } else {
        Ok(NamedFile::open(blob_path)?)
    }
}

fn manifests(config: HandlerConfig, info: web::Path<(String, String, String)>) -> HttpResponse {
    // https://docs.docker.com/registry/spec/api/#errors
    // (errors are 400s)
    let layers = match build_layers(&config, &info.0, &info.1, &info.2) {
        Ok(x) => x,
        Err(Error::FetchFailed) => {
            return HttpResponse::InternalServerError().body("git fetch failed")
        }
        _ => return HttpResponse::InternalServerError().body("other layer creation error"),
    };

    let rootfs = RootFSContainer {
        architecture: "amd64".to_string(),
        created: "1970-01-01T00:00:01Z".to_string(),
        os: "linux".to_string(),
        rootfs: RootFS {
            type_: "layers".to_string(),
            diff_ids: layers.iter().map(|x| x.digest.clone()).collect(),
        },
    };

    // create a blob for the rootfs object
    let rootfs_blob = serde_json::to_vec(&rootfs).unwrap();

    let mut hasher = Sha256::new();
    hasher.input(&rootfs_blob);
    let digest = format!("sha256:{}", hasher.result_str());

    // Store rootfs json in blob store
    std::fs::write(config.blob_root.join(&digest), &rootfs_blob).unwrap();

    // TODO "sha256:" +
    let manifest = DockerManifestV2 {
        schema_version: 2,
        media_type: "application/vnd.docker.distribution.manifest.v2+json".to_string(),
        config: DockerManifestV2Config {
            media_type: "application/vnd.docker.container.image.v1+json".to_string(),
            size: rootfs_blob.len(),
            digest,
        },
        layers,
    };

    HttpResponse::Ok()
        .content_type("application/vnd.docker.distribution.manifest.v2+json")
        .json(manifest)
}

fn v2() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body("")
}

const USAGE: &str = "
Usage: nixcr --blob-root BLOBROOT --repo-root REPOROOT --repo REPO...

Options:
    --blob-root BLOBROOT  Where to store blobs (e.g. persistent disk)
    --repo-root REPOROOT  Where to store cloned repos (e.g. persitent disk)
    --repo REPO            One repo config in the form url,key-path
";

#[derive(Deserialize, Debug)]
struct Args {
    flag_repo: Vec<String>,
    flag_repo_root: String,
    flag_blob_root: String,
}

#[derive(Debug)]
struct RepoConfig {
    // todo - can this be done with str and a lifetime annotation?
    // could not figure out how that interacts with the App || move..
    lookup_key: String, // local reference path
    url: String,
    deploy_key_path: Option<std::path::PathBuf>,
}

impl RepoConfig {
    /// parse a repo string of the form URL,key-path or just URL
    /// into a repo
    fn parse(s: &str) -> RepoConfig {
        let parts: Vec<&str> = s.split(',').collect();
        match parts.as_slice() {
            [lookup_key, url] => RepoConfig {
                lookup_key: String::from(*lookup_key),
                url: String::from(*url),
                deploy_key_path: None,
            },
            [lookup_key, url, deploy_key_path] => RepoConfig {
                lookup_key: String::from(*lookup_key),
                url: String::from(*url),
                deploy_key_path: Some(std::path::PathBuf::from(deploy_key_path)),
            },
            _ => panic!("no"),
        }
    }
    /// returns path for where to clone / fetch the repo
    fn git_dir(&self) -> std::path::PathBuf {
        let re = regex::Regex::new(r"[^A-Za-z_]").unwrap();
        // anything that's not a letter gets replaced with _
        // this can lead to collisions which is not great but OK for this use case?
        std::path::PathBuf::from(re.replace_all(&self.url, "_").into_owned())
    }
}

fn main() -> std::io::Result<()> {
    env_logger::init();
    let args: Args = docopt::Docopt::new(USAGE).unwrap().deserialize().unwrap();
    let repo_configs: HashMap<String, RepoConfig> = args
        .flag_repo
        .iter()
        .map(|x| {
            let rc = RepoConfig::parse(&x);
            (String::from(&rc.lookup_key), rc)
        })
        .collect();

    let config = web::Data::new(Arc::new(Config {
        blob_root: std::path::PathBuf::from(&args.flag_blob_root),
        repo_root: std::path::PathBuf::from(&args.flag_repo_root),
        repo_configs,
    }));

    std::fs::create_dir_all(&config.blob_root).unwrap();
    std::fs::create_dir_all(&config.repo_root).unwrap();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .register_data(config.clone())
            .route("/v2/", web::get().to(v2))
            .route(
                "/v2/{lookup_key}/{commit}/manifests/{reference}",
                web::get().to(manifests),
            )
            .route(
                "/v2/{lookup_key}/{commit}/blobs/{reference}",
                web::get().to(blobs),
            )
    })
    .bind("127.0.0.1:8888")?
    .run()
}
