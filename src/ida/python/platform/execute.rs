//! Execute steps in order; command failures are results, file failures propagate.

use tokio::process::Command;

use super::plan::{Action, Kind, Plan, Step};
use crate::error::Result;
use crate::ida::python::output;
use crate::util::strings::python_trim;

pub(crate) struct StepResult {
    pub kind: Kind,
    pub success: bool,
    pub skipped: bool,
    pub message: String,
}

pub(crate) async fn execute(plan: &Plan) -> Result<Vec<StepResult>> {
    let mut results = Vec::new();
    for step in &plan.steps {
        results.push(execute_step(step).await?);
    }
    Ok(results)
}

async fn execute_step(step: &Step) -> Result<StepResult> {
    let (success, skipped, message) = match &step.action {
        Action::File {
            path,
            content,
        } => {
            let (skipped, message) = super::files::apply(step.kind, path, content)?;
            (true, skipped, message)
        }
        Action::Command {
            program,
            args,
        } => {
            let (success, message) = run(program, args, &step.description).await;
            (success, false, message)
        }
    };
    Ok(StepResult {
        kind: step.kind,
        success,
        skipped,
        message,
    })
}

async fn run(program: &str, args: &[String], description: &str) -> (bool, String) {
    match output(Command::new(program).args(args), 60).await {
        Ok(result) if result.status.success() => (true, description.into()),
        Ok(result) => {
            let bytes = if result.stderr.is_empty() {
                &result.stdout
            } else {
                &result.stderr
            };
            (
                false,
                format!(
                    "{} failed (exit {}): {}",
                    std::iter::once(program)
                        .chain(args.iter().map(String::as_str))
                        .collect::<Vec<_>>()
                        .join(" "),
                    result.status.code().unwrap_or(1),
                    python_trim(&String::from_utf8_lossy(bytes))
                ),
            )
        }
        Err(error) => (false, format!("Failed to run {program}: {error}")),
    }
}

pub(crate) async fn verify(plan: &Plan, applied: bool) -> Option<bool> {
    let (mut command, timeout) = if cfg!(windows) {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            &format!("[Environment]::GetEnvironmentVariable(\"{}\", \"User\")", plan.env_var_name),
        ]);
        (command, 15)
    } else {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", &format!("echo \"${}\"", plan.env_var_name)]);
        (command, 5)
    };
    // Model upstream's process-environment update explicitly for this child.
    // Mutating libc's environment inside Tokio's multithreaded runtime is unsafe.
    if applied {
        command.env(&plan.env_var_name, &plan.env_var_value);
    }
    let result = output(&mut command, timeout).await.ok()?;
    if cfg!(windows) && !result.status.success() {
        return None;
    }
    Some(
        result.status.success()
            && python_trim(&String::from_utf8_lossy(&result.stdout)) == plan.env_var_value,
    )
}
