//! Show a concrete configuration plan before confirmation and report every result.

use std::path::Path;

use super::ui::Ui;
use crate::error::Result;
use crate::ida::python::{env_var, platform};

#[derive(Default)]
pub(super) struct Outcome {
    pub configured: bool,
    pub via: Option<String>,
}

impl Outcome {
    fn from_results(results: &[platform::StepResult]) -> Self {
        if results.iter().any(|result| !result.success) {
            return Self::default();
        }
        let kinds: Vec<_> = results
            .iter()
            .filter(|result| !result.skipped)
            .map(|result| result.kind.name())
            .collect();
        Self {
            configured: true,
            via: (!kinds.is_empty()).then(|| kinds.join(", ")),
        }
    }
}

pub(super) async fn run(executable: &Path, ui: &Ui) -> Result<Outcome> {
    const NAME: &str = "IDAPYTHON_VENV_EXECUTABLE";
    let value = executable.to_string_lossy();
    let plan = platform::build(&platform::Context::detect()?, NAME, &value);
    if let Some(current) = env_var(NAME)
        && Path::new(&current) != executable
    {
        ui.print(&format!("${NAME} is currently {current}. It must change."));
    }
    ui.print(&format!(
        "\nTo make IDA use this environment, {NAME} must be set in your login session."
    ));
    ui.print("HCLI will:");
    for (index, step) in plan.steps.iter().enumerate() {
        ui.print(&format!("  {}. {}", index + 1, step.description));
        if let platform::Action::File {
            path,
            ..
        } = &step.action
        {
            ui.print(&format!("     {}", path.display()));
        }
    }
    for warning in &plan.warnings {
        ui.print(&format!("  Warning: {warning}"));
    }
    if !ui.confirm("\nApply these changes?")? {
        ui.print(&format!("\n{}", plan.manual_instructions));
        return Ok(Outcome::default());
    }
    let results = platform::execute(&plan).await?;
    for result in &results {
        ui.print(&format!("  {}", result.message));
    }
    let outcome = Outcome::from_results(&results);
    if !outcome.configured {
        ui.print("\nSome steps failed. Review the output above.");
        ui.print(&format!("Manual instructions:\n{}", plan.manual_instructions));
        return Ok(Outcome::default());
    }
    match platform::verify(&plan, results.iter().any(|result| result.success)).await {
        Some(true) => ui.print(&format!("\n{NAME} is set and verified.")),
        Some(false) => {
            ui.print(&format!("\n{NAME} was written but could not be verified in a subprocess."))
        }
        None => (),
    }
    ui.print(if plan.needs_logout() {
        "Log out and back in for all changes to take effect."
    } else {
        "Restart IDA and any open terminals for the change to take effect."
    });
    Ok(outcome)
}

#[cfg(test)]
mod tests;
