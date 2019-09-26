// Docker v2 registry that shells out to nix to
// build layers.
// Could be better:
// * non-unwrap error handling
// * layer packing
// * more consistent naming for the docker JSON structure]
#[macro_use] extern crate log;
use actix_files::NamedFile;
use actix_web::{web, App, HttpServer, HttpResponse};
use serde::{Serialize};
use std::vec::{Vec};
use serde_json;
use crypto::sha2::{Sha256};
use crypto::digest::{Digest};
use std::sync::Arc;
use actix_web::middleware::Logger;

mod store;


#[derive(Debug)]
struct Config<'a> {
    /// Directory to store and serve blobs. Must exist.
    blob_root: &'a std::path::Path,
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


#[derive(Serialize)]
#[derive(Debug)]
struct RootFS {
    #[serde(rename = "type")]
    type_: String,
    diff_ids: Vec<String>, // NB no camel case
}


#[serde(rename_all = "camelCase")]
#[derive(Serialize)]
struct LayerMeta {
    media_type: String,
    size: usize, // size of layer.tar
    digest: String, // compressed
}


#[derive(Debug)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RootFSContainer {
    architecture: String,
    created: String,
    os: String,
    rootfs: RootFS,
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

fn build_layers(config: &Config, attr_path: &str) -> Vec<LayerMeta> {
    let _build = std::process::Command::new("nix-build")
        .arg("/home/tom/src/nixpkgs")
        .arg("-A")
        .arg(attr_path)
        .output()
        .expect("build failed");
    // TODO - rename out result so we can query it?
    //     alternatively query output from _build.
    let query = std::process::Command::new("nix-store")
        .arg("-qR")
        .arg("result")
        .output()
        .expect("query failed");

    // Dumb even-sized layer chunker, I believe the query
    // returns in dependency order so this chunker will at least
    // pack some stuff together correctly.
    let mut all_paths = Vec::new();
    for x in query.stdout.split(|c| *c == 0x0au8) {
        if x.len() == 0 { continue };
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
            archive_builder.append_dir_all(x.strip_prefix("/").unwrap(), x).unwrap();
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
    layers
}


fn blobs(config: web::Data<std::sync::Arc<Config>>, info: web::Path<(String, String)>) -> actix_web::Result<NamedFile> {
    let blob_path = config.blob_root.join(info.1.clone());
    if !blob_path.is_file() {
        Err(actix_web::error::ErrorNotFound(""))
    } else {
        Ok(NamedFile::open(blob_path)?)
    }
}


fn manifests(config: web::Data<std::sync::Arc<Config>>, info: web::Path<(String, String)>) -> HttpResponse {
    // tar_path = _git_checkout(name)
    // attribute_path = reference.split('.')
    // m['layers'] = list(_build_layers(attribute_path, tar_path))

    let layers = build_layers(&config, "hello");

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
            digest: digest,
        },
        layers: layers,
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


fn main() -> std::io::Result<()>  {
    env_logger::init();

    let config = web::Data::new(Arc::new(Config {
        blob_root: std::path::Path::new("/tmp/blobs"),
    }));
    HttpServer::new(
        move || App::new()
            .wrap(Logger::default())
            .register_data(config.clone())
            .route("/v2/", web::get().to(v2))
            .route("/v2/{name:.*?}/manifests/{reference}", web::get().to(manifests))
            .route("/v2/{name:.*?}/blobs/{reference}", web::get().to(blobs))
    ).bind("127.0.0.1:8888")?
    .run()
}
