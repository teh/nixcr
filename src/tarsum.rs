// use std::io::prelude::*;
use tar::Archive;
use std::fs::File;
use sha2::{Sha256, Digest};


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
        header.entry_type().as_byte(),
        link_name,
        match header.username().unwrap() { Some(x) => x, None => ""},
        match header.groupname().unwrap() { Some(x) => x, None => ""},
        match header.device_major() { Ok(Some(x)) => x, _ => 0},
        match header.device_minor() { Ok(Some(x)) => x, _ => 0},
    )
}


fn open() {
    let file = File::open("/nix/store/rnybsc38hj6gq6fd31b521hf735iyjjs-blarg-customisation-layer/layer.tar").unwrap();
    let mut a = Archive::new(file);

    for file in a.entries().unwrap() {
        // Make sure there wasn't an I/O error
        let mut file = file.unwrap();

        // Inspect metadata about the file

        canonical_header_representation(&file.header());

        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher);
        let result = hasher.result();
        println!("{:?}, {:x}", file.header().path().unwrap(), result);
    }
}



#[cfg(test)]
mod tests {
   // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_open() {
        open();
    }
}
