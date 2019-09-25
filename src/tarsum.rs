// NB it looks like tarsum is bad and probably isn't even needed for this service:
// https://lwn.net/Articles/628343/
// Might delete at some point.
//
// Partial and fragile implementation of tarsum
// https://github.com/moby/moby/blob/master/pkg/tarsum/tarsum_spec.md
// known missing:
// * xattr
// * dealing with duplicated entries
//
// Could be better
// * error handling instead of unwrap
use tar::Archive;
use std::fs::File;
use sha2::{Sha256, Digest};


fn canonical_header_representation<R: std::io::Read>(entry: &tar::Entry<R>) -> String {
    let link_name = match entry.link_name_bytes() { Some(x) => x.into_owned(), None => Vec::new() };
    let link_name_str = std::str::from_utf8(&link_name).unwrap();
    let header = entry.header();

    // BUG! If a directory linkname seems to be missing the last part of the directory name?
    // /asn1crypto-0.24.0.dist-info in this case..
    format!("name{}mode{}uid{}gid{}size{}typeflag{}linkname{}uname{}gname{}devmajor{}devminor{}",
        header.path().unwrap().display(),
        header.mode().unwrap(),
        header.uid().unwrap(),
        header.gid().unwrap(),
        header.size().unwrap(),
        header.entry_type().as_byte() as char,
        link_name_str,
        // uname and gname seems to be set to "" in the go implementation
        "", // match header.username().unwrap() { Some(x) => x, None => ""},
        "", // match header.groupname().unwrap() { Some(x) => x, None => ""},
        match header.device_major() { Ok(Some(x)) => x, _ => 0},
        match header.device_minor() { Ok(Some(x)) => x, _ => 0},
    )
}


fn tarsum<R: std::marker::Sized + std::io::Read>(a: &mut tar::Archive<R>) -> String {
    let mut sums = Vec::new();

    for file in a.entries().unwrap() {
        let mut file = file.unwrap();

        let mut hasher = Sha256::new();
        hasher.input(canonical_header_representation(&file));
        std::io::copy(&mut file, &mut hasher).unwrap();
        let result = hasher.result();

        sums.push(format!("{:x}", result));
    }
    sums.sort();
    let full_sum = sha2::Sha256::digest(sums.join("").as_bytes());
    format!("tarsum.v1+sha256:{:x}", full_sum)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn _open() -> String {
        let file = File::open("/nix/store/rnybsc38hj6gq6fd31b521hf735iyjjs-blarg-customisation-layer/layer.tar").unwrap();
        let mut a = Archive::new(file);
        tarsum(&mut a)
    }

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

        let mut archive_builder = tar::Builder::new(Vec::new());
        archive_builder.append(&test_header, std::io::empty()).unwrap();
        let archive_bytes = archive_builder.into_inner().unwrap();
        let mut archive = Archive::new(&archive_bytes[..]);

        assert_eq!(
            tarsum(&mut archive),
            "tarsum.v1+sha256:6ffd43a1573a9913325b4918e124ee982a99c0f3cba90fc032a65f5e20bdd465"
        );

        // testing the following needs file system access
        // println!("{}", _open());
    }
}
