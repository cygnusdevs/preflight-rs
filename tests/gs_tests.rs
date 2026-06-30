// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{os::unix::fs::PermissionsExt, time::Duration};

use preflight_rs::gs::{parse_inkcov, run_inkcov, InkCoverage};
use tempfile::tempdir;
use tokio::sync::Semaphore;

#[test]
fn parse_inkcov_reads_cmyk_lines_and_numbers_pages() {
    let output = "\
Processing pages 1 through 2.
Page 1
 0.00000  0.01337  0.00000  0.50000 CMYK OK
Page 2
 0.25000  0.00000  0.12500  0.00000 CMYK OK
";

    let parsed = parse_inkcov(output).expect("inkcov output parses");

    assert_eq!(
        parsed,
        vec![
            InkCoverage {
                page: 1,
                c: 0.0,
                m: 0.01337,
                y: 0.0,
                k: 0.5
            },
            InkCoverage {
                page: 2,
                c: 0.25,
                m: 0.0,
                y: 0.125,
                k: 0.0
            }
        ]
    );
}

#[test]
fn parse_inkcov_rejects_output_without_coverage_rows() {
    let err = parse_inkcov("GPL Ghostscript 10.07.1").expect_err("missing rows is invalid");

    assert!(err.to_string().contains("coverage"));
}

#[tokio::test]
async fn run_inkcov_times_out_slow_ghostscript_processes() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("slow-gs");
    std::fs::write(&script, "#!/bin/sh\nsleep 2\n").unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    let result = tokio::time::timeout(
        Duration::from_millis(1500),
        run_inkcov(
            script.to_str().unwrap(),
            b"%PDF-1.7\n%%EOF",
            &Semaphore::new(1),
            Duration::from_millis(250),
        ),
    )
    .await;

    assert!(
        result.is_ok(),
        "Ghostscript process did not time out internally"
    );
    assert!(result.unwrap().is_err());
}
