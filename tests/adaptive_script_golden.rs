//! Golden prompt scenarios (PRD SC-4): natural-language requests where script-style JSON is the
//! expected shape in manual or future CI checks. Each constant is a user request string — not an assertion target.

/// Parse multi-line CSV and print column sums — clearer as a short Python script than `awk`.
pub const SCENARIO_CSV_AGGREGATE: &str =
    "Sum the third column of data.csv where the first column starts with 'A' and print one total per group.";

/// JSON transform with error handling — Node or Python beats nested shell `jq` + pipes for some models.
pub const SCENARIO_NESTED_JSON_TRANSFORM: &str =
    "Read package.json, double every numeric dependency version patch component, and write to stdout as formatted JSON.";

/// Cross-platform path logic — script avoids brittle `sed`/`grep` differences.
pub const SCENARIO_PATH_REWRITE: &str =
    "Walk the current directory tree, find files named '*.bak', and print their paths with the .bak removed from the name.";

/// Algorithmic task — loop with state fits a runtime, not a one-liner.
pub const SCENARIO_FIZZBUZZ_STYLE: &str =
    "Print numbers 1 through 100, but for multiples of 3 print 'fizz', multiples of 5 'buzz', both 'fizzbuzz'.";

/// Ruby one-off text munging when Ruby is on PATH (doctor lists it).
pub const SCENARIO_RUBY_TEXT: &str =
    "Read stdin, replace every occurrence of the word 'TODO' with 'DONE' case-sensitively, write to stdout.";

#[test]
fn golden_scenario_strings_are_nonempty() {
    for (name, s) in [
        ("csv", SCENARIO_CSV_AGGREGATE),
        ("json", SCENARIO_NESTED_JSON_TRANSFORM),
        ("paths", SCENARIO_PATH_REWRITE),
        ("fizz", SCENARIO_FIZZBUZZ_STYLE),
        ("ruby", SCENARIO_RUBY_TEXT),
    ] {
        assert!(
            s.len() > 20,
            "scenario {name} should be a substantive prompt"
        );
    }
}
