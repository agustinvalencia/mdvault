//! Variable specification and metadata for templates, captures, and macros.
//!
//! This module defines the schema for declaring variables with:
//! - Prompts (human-readable text shown when collecting input)
//! - Defaults (static or computed with date math)
//! - Required/optional status
//!
//! Variables can be extracted from frontmatter in templates/captures/macros.

pub mod datemath;
pub mod types;

pub use datemath::{
    DateBase, DateExpr, DateMathError, DateOffset, Direction, DurationUnit,
    evaluate_date_expr, is_date_expr, parse_date_expr, try_evaluate_date_expr,
};
pub use types::{
    VarMetadata, VarSpec, VarsMap, collect_all_variables, extract_variable_names,
};
