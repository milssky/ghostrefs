mod backend;
mod python;
mod registry;

pub use backend::Language;
pub use registry::{detect, excludes_directory, from_path, supports_path};
