mod atomic_file;
mod image_store;
mod import;
mod manifest_store;
mod recovery;
mod session;

pub use image_store::*;
pub use import::*;
pub use manifest_store::*;
pub use recovery::*;
pub use session::*;

#[cfg(test)]
pub(crate) fn create_legacy_segmented_session(
    root: &std::path::Path,
    filename: &str,
    ratio: &str,
    fit: &str,
) -> anyhow::Result<SessionStore> {
    let session = SessionStore::create(root, filename, ratio, fit)?;
    let journal = session.directory.join("session.json");
    let mut data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&journal).unwrap()).unwrap();
    data["version"] = 2.into();
    std::fs::write(&journal, serde_json::to_vec(&data).unwrap()).unwrap();
    SessionStore::open(&session.directory)
}
