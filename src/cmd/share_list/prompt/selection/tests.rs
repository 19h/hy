use serde_json::{Value, json};

use super::*;

fn labels() -> Vec<String> {
    ["Alpha (one) - 1.0 KB", "Beta (two) - 2.0 KB", "Gamma (three) - 3.0 KB"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn apply(model: &mut Selection, action: u8) {
    model.apply(match action {
        0 => Edit::Type('T'),
        1 => Edit::Type('Z'),
        2 => Edit::Erase,
        3 => Edit::Next,
        4 => Edit::Previous,
        5 => Edit::Toggle,
        6 => Edit::ToggleAll,
        7 => Edit::Invert,
        _ => unreachable!(),
    });
}

#[test]
fn selections_survive_filters_and_submit_in_original_order() {
    let mut model = Selection::new(&labels(), &[0, 1, 2]);
    model.apply(Edit::Next);
    model.apply(Edit::Toggle);
    model.apply(Edit::Type('A'));
    model.apply(Edit::Type('l'));
    assert_eq!(model.visible, [0]);
    assert_eq!(model.selected(), [1]);
    model.apply(Edit::Toggle);
    assert_eq!(model.selected(), [0, 1]);
    model.apply(Edit::ToggleAll);
    assert_eq!(model.selected(), [0, 1, 2]);
    model.apply(Edit::ToggleAll);
    assert!(model.selected().is_empty());
    model.apply(Edit::Invert);
    assert_eq!(model.selected(), [0, 1, 2]);
    model.apply(Edit::Type('Z'));
    assert!(!model.found_matches);
    assert_eq!(model.visible, [0, 1, 2]);
    model.apply(Edit::Previous);
    assert_eq!(model.cursor, 2);
    model.apply(Edit::Erase);
    assert_eq!(model.visible, [0]);
    assert_eq!(model.cursor, 0);
}

#[test]
fn checkbox_state_sequences_match_upstream_questionary() {
    compare_sequences(
        &[0, 1, 2],
        "fceae4acf0e35442c1e0e5745c58387ea2bf88f026a6a5e4970d6a48d547a324",
    );
}

#[test]
fn equal_choice_values_preserve_selection_multiplicity() {
    compare_sequences(
        &[0, 0, 1],
        "08372d5e41d7622a3a7b9336ec675c5b2455a7931a101685042ef74be8ed645c",
    );
}

fn compare_sequences(groups: &[usize], expected_digest: &str) {
    let mut sequences = vec![Vec::<u8>::new()];
    let mut frontier = vec![Vec::<u8>::new()];
    for _ in 0..4 {
        frontier = frontier
            .iter()
            .flat_map(|prefix| {
                (0..8).map(move |action| {
                    let mut sequence = prefix.clone();
                    sequence.push(action);
                    sequence
                })
            })
            .collect();
        sequences.extend(frontier.iter().cloned());
    }
    assert_eq!(sequences.len(), 4681);
    let expected: Vec<_> = sequences
        .iter()
        .map(|sequence| {
            let mut model = Selection::new(&labels(), groups);
            for &action in sequence {
                apply(&mut model, action);
            }
            json!([model.visible, model.selected(), model.cursor, model.query, model.found_matches])
        })
        .collect();
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(digest, expected_digest);
    if let Some(python) = std::env::var_os("HY_TEST_SHARE_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import json, sys
from prompt_toolkit.keys import Keys
from questionary import Choice
from questionary.prompts.common import InquirerControl
data = json.load(sys.stdin)
results = []
for sequence in data['sequences']:
    control = InquirerControl([Choice(label, value=data['groups'][i]) for i, label in enumerate(data['labels'])], show_selected=True)
    indices = {id(choice): i for i, choice in enumerate(control.choices)}
    for action in sequence:
        if action < 2:
            control.add_search_character('TZ'[action])
        elif action == 2:
            control.add_search_character(Keys.Backspace)
        elif action == 3:
            control.select_next()
        elif action == 4:
            control.select_previous()
        elif action == 5:
            value = control.get_pointed_at().value
            if value in control.selected_options:
                control.selected_options.remove(value)
            else:
                control.selected_options.append(value)
        elif action == 6:
            all_selected = True
            for choice in control.choices:
                if choice.value not in control.selected_options:
                    control.selected_options.append(choice.value)
                    all_selected = False
            if all_selected:
                control.selected_options = []
        else:
            control.selected_options = [choice.value for choice in control.choices if choice.value not in control.selected_options]
    visible = [indices[id(choice)] for choice in control.filtered_choices]
    results.append([visible, [indices[id(choice)] for choice in control.get_selected_values()], control.pointed_at, control.search_filter or '', not control.search_filter or control.found_in_search])
print(json.dumps(results))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let input = json!({"labels": labels(), "sequences": sequences, "groups": groups});
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "sequence {:?}", sequences[index]);
        }
        assert_eq!(actual.len(), expected.len());
    }
}
