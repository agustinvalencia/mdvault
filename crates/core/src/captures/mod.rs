pub mod discovery;
pub mod hooks;
pub mod lua_loader;
pub mod repository;
pub mod types;

// Re-export primary API
pub use hooks::{run_after_insert_hook, run_before_insert_hook, AfterInsertResult, BeforeInsertResult};
pub use lua_loader::load_capture_from_lua;
pub use repository::CaptureRepository;
pub use types::{
    CaptureDiscoveryError, CaptureFormat, CaptureInfo, CapturePosition, CaptureRepoError,
    CaptureSpec, CaptureTarget, LoadedCapture,
};
