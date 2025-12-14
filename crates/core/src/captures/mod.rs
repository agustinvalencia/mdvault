pub mod discovery;
pub mod repository;
pub mod types;

// Re-export primary API
pub use repository::CaptureRepository;
pub use types::{
    CaptureDiscoveryError, CaptureInfo, CapturePosition, CaptureRepoError, CaptureSpec,
    CaptureTarget, LoadedCapture,
};
