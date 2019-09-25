use actix_web::{web, App, HttpServer, HttpResponse};
use serde::{Serialize};
use std::vec::{Vec};
use serde_json;
use sha2::{Sha256, Digest};

mod tarsum;


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
}

#[derive(Serialize)]
struct RootFS {
    type_: String,
    diff_ids: Vec<String>, // NB no camel case
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RootFSContainer {
    architecture: String,
    created: String,
    os: String,
    rootfs: RootFS,
}


fn build_layers()  {
    let _build = std::process::Command::new("nix-build")
        .arg("/home/tom/src/nixpkgs")
        .arg("-A")
        .arg("hello")
        .output()
        .expect("build failed");
    let query = std::process::Command::new("nix-store")
        .arg("-qR")
        .arg("result")
        .output()
        .expect("query failed");

    // Dumb chunker
    let mut all_paths = Vec::new();
    for x in query.stdout.split(|c| *c == 0x0au8) {
        if x.len() == 0 { continue };
        all_paths.push(std::path::Path::new(std::str::from_utf8(x).unwrap()));
    }
    let paths_per_layer = all_paths.len() / 100usize + 1;

    let base_path = std::path::Path::new("/tmp/nixcr");

    for chunk in all_paths.chunks(paths_per_layer) {
        // todo new needs to be sth that implements the Write trait other than Vec::new()

         // TODO replace build.tar witih some temp thing
        let mut archive_builder = tar::Builder::new(std::fs::File::create(base_path.join("buid.tar")).unwrap());
        archive_builder.follow_symlinks(false); // keep symlinks in docker

        for x in chunk {
            println!("{:?}", x);
            archive_builder.append_dir_all(x.strip_prefix("/").unwrap(), x).unwrap();
        }
        archive_builder.into_inner().unwrap();
    }

    // partition into buckets
    // layer_meta = {
    //     "mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip", // not sure I need to compress?
    //     "size": size, // size of layer.tar
    //     "digest": digest, // compressed
    //     "layer_sha256": layer_sha256, // uncompressed
    // }

    ()
}


fn manifest(info: web::Path<(String, String)>) -> HttpResponse {
    // tar_path = _git_checkout(name)
    // attribute_path = reference.split('.')
    // m['layers'] = list(_build_layers(attribute_path, tar_path))

    build_layers();

    let rootfs = RootFSContainer {
        architecture: "amd64".to_string(),
        created: "1970-01-01T00:00:01Z".to_string(),
        os: "linux".to_string(),
        rootfs: RootFS {
            type_: "layers".to_string(),
            diff_ids: Vec::new(),
        },
    };

    // create a blob for the rootfs object
    let rootfs_blob = serde_json::to_vec(&rootfs).unwrap();
    let digest = format!("{:x}", Sha256::digest(&rootfs_blob));

    // TODO "sha256:" +
    let manifest = DockerManifestV2 {
        schema_version: 2,
        media_type: "application/vnd.docker.distribution.manifest.v2+json".to_string(),
        config: DockerManifestV2Config {
            media_type: "application/vnd.docker.container.image.v1+json".to_string(),
            size: rootfs_blob.len(),
            digest: format!("sha256:{}", digest),
        },
    };

    HttpResponse::Ok()
        .json(manifest)
}


fn v2() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body("")
}


fn main() -> std::io::Result<()>  {
    HttpServer::new(
        || App::new()
            .route("/v2/", web::get().to(v2))
            .route("/v2/{name:.*?}/manifests/{reference}", web::get().to(manifest))
    ).bind("127.0.0.1:8888")?
    .run()
}
