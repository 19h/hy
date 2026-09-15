//! Virtual-clock tests for the actual startup and analysis polling policies.

use super::*;

fn instance() -> ipc::Instance {
    ipc::Instance {
        pid: 42,
        socket: "fixture".into(),
        idb_path: Some("sample.i64".into()),
    }
}

#[tokio::test(start_paused = true)]
async fn startup_polling_backs_off_from_100_ms_to_a_two_second_cap() {
    let start = tokio::time::Instant::now();
    let mut observations = Vec::new();
    let found = database(f64::INFINITY, || {
        observations.push(start.elapsed());
        std::future::ready(if observations.len() == 12 {
            Some(instance())
        } else {
            None
        })
    })
    .await
    .unwrap();
    assert_eq!(found.pid, 42);
    assert_eq!(
        &observations[..4],
        &[
            Duration::ZERO,
            Duration::from_millis(100),
            Duration::from_millis(250),
            Duration::from_millis(475)
        ]
    );
    let intervals: Vec<_> = observations.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(intervals.iter().all(|interval| *interval <= Duration::from_secs(2)));
    assert_eq!(intervals.last(), Some(&Duration::from_secs(2)));
}

#[tokio::test(start_paused = true)]
async fn startup_timeout_is_checked_between_queries_as_upstream_does() {
    let start = tokio::time::Instant::now();
    let mut calls = 0;
    let error = database(0.21, || {
        calls += 1;
        std::future::ready(None)
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("0.21 s"));
    assert_eq!(calls, 2);
    assert_eq!(start.elapsed(), Duration::from_millis(250));
    for timeout in [0.0, -1.0, f64::NEG_INFINITY, f64::NAN] {
        let result = database(timeout, || -> std::future::Ready<Option<ipc::Instance>> {
            panic!("nonpositive/NaN timeout must not poll")
        })
        .await;
        assert!(result.is_err());
    }
}

#[tokio::test(start_paused = true)]
async fn analysis_has_its_own_five_second_polling_cycle_without_a_startup_deadline() {
    let start = tokio::time::Instant::now();
    let mut responses = [
        json!({"status":"ok","analysis_complete":false}),
        json!({"status":"timeout"}),
        json!({"status":"cancelled"}),
        json!({"status":"ok","analysis_complete":true}),
    ]
    .into_iter();
    poll_analysis(|| {
        std::future::ready(Ok(responses.next().expect("no queries after completion")))
    })
    .await
    .unwrap();
    assert_eq!(start.elapsed(), Duration::from_secs(15));
}

#[tokio::test(start_paused = true)]
async fn analysis_errors_stop_immediately() {
    let start = tokio::time::Instant::now();
    let error =
        poll_analysis(|| std::future::ready(Ok(json!({"status":"error","message":"closed"}))))
            .await
            .unwrap_err();
    assert!(error.to_string().contains("closed"));
    assert_eq!(start.elapsed(), Duration::ZERO);
    let error = poll_analysis(|| std::future::ready(Err(Error::Other("transport failed".into()))))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("transport failed"));
}

#[test]
fn analysis_status_and_truthiness_follow_the_upstream_response_contract() {
    for value in [json!(true), json!(1), json!(-1), json!("false"), json!([null]), json!({"key":0})]
    {
        assert!(analysis_complete(&json!({"status":"ok","analysis_complete":value})).unwrap());
    }
    for value in [json!(false), json!(0), json!(0.0), Value::Null, json!(""), json!([]), json!({})]
    {
        assert!(!analysis_complete(&json!({"status":"ok","analysis_complete":value})).unwrap());
    }
    for status in [json!("timeout"), json!("cancelled"), json!("unknown"), Value::Null, json!(0)] {
        assert!(!analysis_complete(&json!({"status":status,"analysis_complete":true})).unwrap());
    }
    assert!(!analysis_complete(&json!({"status":"ok","complete":true})).unwrap());
    assert!(analysis_complete(&json!({"analysis_complete":true})).is_err());
}
