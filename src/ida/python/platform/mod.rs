//! Plan and persist IDAPython configuration for terminal and desktop sessions.

mod detection;
mod execute;
mod files;
mod plan;

pub(super) use detection::Context;
pub(super) use execute::{StepResult, execute, verify};
pub(super) use plan::{Action, build};

#[cfg(test)]
pub(super) use plan::Kind;

#[cfg(test)]
mod tests;
