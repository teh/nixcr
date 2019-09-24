// Partial implementation of tarsum
// https://github.com/moby/moby/blob/master/pkg/tarsum/tarsum_spec.md
// known missing:
// * xattr

use tar::Archive;
use std::fs::File;
use sha2::{Sha256, Digest};
use hex;
use std::io::prelude::*;

fn canonical_header_representation(header: &tar::Header) -> String {
    let link_name = match header.link_name().unwrap() {
        Some(x) => x.into_owned().display().to_string(),
        None => "".to_string(),
    };
    format!("name{}mode{}uid{}gid{}size{}typeflag{}linkname{}uname{}gname{}devmajor{}devminor{}",
        header.path().unwrap().display(),
        header.mode().unwrap(),
        header.uid().unwrap(),
        header.gid().unwrap(),
        header.size().unwrap(),
        header.entry_type().as_byte() as char,
        link_name,
        // uname and gname seems to be set to "" in the go implementation
        "", // match header.username().unwrap() { Some(x) => x, None => ""},
        "", // match header.groupname().unwrap() { Some(x) => x, None => ""},
        match header.device_major() { Ok(Some(x)) => x, _ => 0},
        match header.device_minor() { Ok(Some(x)) => x, _ => 0},
    )
}


fn open() -> String {
    let file = File::open("/nix/store/rnybsc38hj6gq6fd31b521hf735iyjjs-blarg-customisation-layer/layer.tar").unwrap();
    let mut a = Archive::new(file);
    tarsum(&mut a)
}

fn tarsum<R: std::marker::Sized + std::io::Read>(a: &mut tar::Archive<R>) -> String {
    let mut sums = Vec::new();

    for file in a.entries().unwrap() {
        // Make sure there wasn't an I/O error
        let mut file = file.unwrap();

        // Inspect metadata about the file
        let mut hasher = Sha256::new();
        hasher.input(canonical_header_representation(&file.header()));
        std::io::copy(&mut file, &mut hasher);
        let result = hasher.result();

        sums.push(format!("{:x}", result));
        // println!("{:?}, {:x}", file.header().path().unwrap(), result);
    }
    sums.sort();
    let full_sum = sha2::Sha256::digest(sums.join("").as_bytes());
    format!("tarsum.v1+sha256:{:x}", full_sum)
}


#[cfg(test)]
mod tests {
   // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_header() {
        // $  /nix/store/69b2didvk2086qsn56l738dwbim6kz4v-tarsum/bin//tarsum </tmp/foo.txt
        // tarsum.v1+sha256:6ffd43a1573a9913325b4918e124ee982a99c0f3cba90fc032a65f5e20bdd465
		let mut test_header = tar::Header::new_gnu();
        test_header.set_path("file.txt").unwrap();
        test_header.set_size(0);
        test_header.set_mode(0);
        test_header.set_uid(0);
        test_header.set_gid(0);
        test_header.set_entry_type(tar::EntryType::Regular);
        test_header.set_device_minor(0).unwrap();
        test_header.set_device_major(0).unwrap();
        test_header.set_cksum();
        assert_eq!(
            canonical_header_representation(&test_header),
            "namefile.txtmode0uid0gid0size0typeflag0linknameunamegnamedevmajor0devminor0"
        );

        let mut archive_builder = tar::Builder::new(Vec::new());
        archive_builder.append(&test_header, std::io::empty()).unwrap();
        let archive_bytes = archive_builder.into_inner().unwrap();
        let mut archive = Archive::new(&archive_bytes[..]);

        assert_eq!(
            tarsum(&mut archive),
            "tarsum.v1+sha256:6ffd43a1573a9913325b4918e124ee982a99c0f3cba90fc032a65f5e20bdd465"
        )
    }
}
