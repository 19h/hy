use std::io::Write;
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

type Key = [u8; 2];

fn less(left: &Key, right: &Key) -> bool {
    let component = usize::from(left[0] == right[0]);
    let (left, right) = (left[component], right[component]);
    left != right && left & right == left
}

#[test]
fn short_partial_orders_match_cpython() {
    let mut cases = Vec::new();
    let keys = [[0, 1], [1, 0], [2, 0], [3, 0]];
    for length in 0..=6 {
        for mut code in 0..4usize.pow(length) {
            let mut values = Vec::new();
            for _ in 0..length {
                values.push(keys[code % 4]);
                code /= 4;
            }
            cases.push(values);
        }
    }
    assert_eq!(cases.len(), 5461);
    verify(&cases, "a7935c8e9c76b8542091493ee4c033c1a9727e8166e431555bf5a00b57122dd9");
}

#[test]
fn long_partial_orders_match_cpython_run_and_merge_boundaries() {
    let mut cases = Vec::new();
    for length in [31, 32, 63, 64, 65, 127, 128, 129, 255, 256, 511, 1024] {
        for seed in 1..=64 {
            let mut random =
                (seed as u64).wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(length as u64);
            for pattern in 0..4 {
                let values = (0..length)
                    .map(|index| {
                        random ^= random << 13;
                        random ^= random >> 7;
                        random ^= random << 17;
                        match pattern {
                            0 => [(random & 7) as u8, ((random >> 8) & 7) as u8],
                            1 => [(random & 255) as u8, ((random >> 8) & 255) as u8],
                            2 => [((1u16 << (index % 9)) - 1) as u8, 0],
                            _ => [((1u16 << (8 - index % 9)) - 1) as u8, (random & 3) as u8],
                        }
                    })
                    .collect();
                cases.push(values);
            }
        }
    }
    assert_eq!(cases.len(), 3072);
    verify(&cases, "6f180536e922fb84ea36179d680855a0a36f66f82b228948c136248c9bb1f0d2");
}

fn verify(cases: &[Vec<Key>], digest: &str) {
    let expected: Vec<Vec<usize>> = cases
        .iter()
        .map(|keys| {
            let mut order: Vec<_> = (0..keys.len()).collect();
            super::sort_by(&mut order, |left, right| less(&keys[*left], &keys[*right]));
            order
        })
        .collect();
    if let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") {
        let mut child = Command::new(python)
            .args(["-I", "-B", "-c", include_str!("reference.py")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let actual: Vec<Vec<usize>> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(actual, expected, "case {index}: {:?}", cases[index]);
        }
    }
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
}
