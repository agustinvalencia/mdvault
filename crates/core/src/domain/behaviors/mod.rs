//! Behavior implementations for first-class note types.

mod custom;
mod daily;
mod project;
mod task;
mod weekly;
mod zettel;

pub use custom::CustomBehavior;
pub use daily::DailyBehavior;
pub use project::ProjectBehavior;
pub use task::TaskBehavior;
pub use weekly::WeeklyBehavior;
pub use zettel::ZettelBehavior;
