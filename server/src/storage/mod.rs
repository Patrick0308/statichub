mod trait_;
mod filesystem;

pub use trait_::{FileInfo, Storage, StorageError};
pub use filesystem::FilesystemStorage;
