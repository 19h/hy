use super::Step;

pub(super) fn all() -> Vec<Vec<Step>> {
    let mut cases = Vec::new();
    for first in [200, 204, 301, 400, 403, 404, 429, 500, 501, 502, 503, 504, 505] {
        for second in [200, 204, 301, 400, 403, 404, 429, 500, 501, 502, 503, 504, 505] {
            cases.push(vec![Step::http(first), Step::http(second), Step::http(200)]);
        }
    }
    let failures = [
        Step::http(403),
        Step::http(429),
        Step::http(503),
        Step::Transient,
        Step::Timeout,
        Step::Terminal,
    ];
    for first in &failures {
        for second in &failures {
            for third in &failures {
                cases.push(vec![first.clone(), second.clone(), third.clone(), Step::http(200)]);
            }
        }
    }
    for count in [3, 4, 5, 6] {
        for step in [Step::http(403), Step::http(429), Step::http(503), Step::Transient] {
            let mut sequence = vec![step; count];
            sequence.push(Step::http(200));
            cases.push(sequence);
        }
    }
    let mut maximum = Vec::new();
    for _ in 0..4 {
        maximum.extend(vec![Step::http(403); 4]);
        maximum.push(Step::http(503));
    }
    cases.push(maximum);
    for status in [403, 429] {
        for value in [
            "", "0", "-1", "59", "60", "61", "3600", "3601", "+1_024", " 60 ", "\t60\t", "bad",
            "1.5", "١",
        ] {
            cases.push(vec![Step::http(status).header("retry-after", value), Step::http(200)]);
        }
        for digits in [309, 4300, 4301] {
            cases.push(vec![
                Step::http(status).header("retry-after", &"9".repeat(digits)),
                Step::http(200),
            ]);
        }
    }
    for status in [200, 403] {
        for remaining in ["", "0", "1", "2", "3", "-1", "invalid"] {
            for reset in [
                "",
                "invalid",
                "0",
                "1700000001",
                "1700000030",
                "1700000061",
                "1700003600",
                "1700003601",
            ] {
                cases.push(vec![
                    Step::http(status)
                        .header("x-ratelimit-remaining", remaining)
                        .header("x-ratelimit-reset", reset),
                    Step::http(200),
                ]);
            }
        }
        for digits in [309, 4300, 4301] {
            cases.push(vec![
                Step::http(status)
                    .header("x-ratelimit-remaining", "0")
                    .header("x-ratelimit-reset", &"9".repeat(digits)),
                Step::http(200),
            ]);
        }
    }
    for first in ["", "10", "invalid"] {
        cases.push(vec![
            Step::http(403)
                .header("retry-after", first)
                .header("Retry-After", "600")
                .header("x-ratelimit-reset", "1700000300"),
            Step::http(200),
        ]);
    }
    for status in [200, 403, 429, 500] {
        cases.push(vec![
            Step::http(status)
                .header("retry-after", "60")
                .header("x-ratelimit-remaining", "invalid")
                .header("x-ratelimit-reset", "invalid"),
            Step::http(200),
        ]);
    }
    let mut malformed_final = vec![Step::http(403); 4];
    malformed_final.push(Step::http(429).header("retry-after", "invalid"));
    cases.push(malformed_final);
    cases
}
