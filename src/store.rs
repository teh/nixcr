// Store is resposible for:
// mapping top-level key (e.g. commit) to a manifest
// storing blobs and deciding their URIs (if e.g. on GCS)
//
// TODO - this abstraction doesn't quite feel right, maybe
// the store should also be responsible for on-the-fly layer
// creation with caching as an implementation detail?
trait Store {
    fn add_layer_tar() -> std::io::Result<()>;
}
