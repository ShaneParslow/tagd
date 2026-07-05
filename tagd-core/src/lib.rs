// Serde structs for queries/responses over socket and socket_path
pub mod query;
// Serde structs for tagger info and response
// With "runtime" feature, Tagger trait that can be implemented and run<Tagger>
// generic for handling execution of basic tagger.
pub mod tagger;
