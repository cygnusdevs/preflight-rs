// SPDX-License-Identifier: AGPL-3.0-or-later

use preflight_rs::gs::{parse_inkcov, InkCoverage};

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
