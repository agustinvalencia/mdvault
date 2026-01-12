pub mod discovery;
pub mod lua_loader;
pub mod repository;
pub mod types;

// Re-export primary API
pub use lua_loader::load_capture_from_lua;
pub use repository::CaptureRepository;
pub use types::{
    CaptureDiscoveryError, CaptureFormat, CaptureInfo, CapturePosition, CaptureRepoError,
    CaptureSpec, CaptureTarget, LoadedCapture,
};
