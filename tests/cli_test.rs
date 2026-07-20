use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn bin_cmd() -> assert_cmd::Command {
    cargo_bin_cmd!("sid")
}

fn parse_stdout_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).unwrap()
}

fn assert_failure_stdout_empty(assert: assert_cmd::assert::Assert) {
    let output = assert.failure().get_output().clone();
    assert!(output.stdout.is_empty());
}

fn assert_json_keys(json: &Value, expected: &[&str]) {
    let object = json.as_object().unwrap();
    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();

    let mut expected = expected.to_vec();
    expected.sort_unstable();

    assert_eq!(actual, expected);
}

fn assert_recognized_sid_ref(sid_ref: &str) {
    let chars = sid_ref.chars().collect::<Vec<_>>();
    assert_eq!(chars.len(), 5);
    assert_eq!(chars[0], 's');

    let alpha22 = "abcdefghjkmnpqrtuvwxyz";
    let slop30 = "23456789abcdefghjkmnpqrtuvwxyz";

    assert!(alpha22.contains(chars[1]));
    assert!(slop30.contains(chars[2]));
    assert!(slop30.contains(chars[3]));
    assert!(slop30.contains(chars[4]));
}

fn assert_json_allocation(
    json: &Value,
    tmp: &Path,
    title: &str,
    slug: &str,
    dry_run: bool,
) -> String {
    assert_json_allocation_in_root(
        json,
        &tmp.canonicalize().unwrap().join("stm"),
        title,
        slug,
        dry_run,
    )
}

fn assert_json_allocation_in_root(
    json: &Value,
    expected_root: &Path,
    title: &str,
    slug: &str,
    dry_run: bool,
) -> String {
    assert_json_keys(
        json,
        &[
            "dry_run", "id", "path", "period", "sid_ref", "slug", "title",
        ],
    );

    assert_eq!(json["title"], title);
    assert_eq!(json["slug"], slug);
    assert_eq!(json["dry_run"], dry_run);

    let period = json["period"].as_str().unwrap();
    assert_eq!(period.len(), 6);
    assert!(period.chars().all(|ch| ch.is_ascii_digit()));

    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert_recognized_sid_ref(sid_ref);

    let id = json["id"].as_str().unwrap();
    assert_eq!(id, format!("{period}_{sid_ref}_{slug}"));

    let path = json["path"].as_str().unwrap();
    assert_eq!(PathBuf::from(path), expected_root.join(id));

    id.to_string()
}

// Fixed period for deterministic allocation tests, injected through the
// hidden `--period` seam.
const TEST_PERIOD: &str = "202606";
const DEFAULT_SCAN_ROOT_NAMES: [&str; 4] = [".pending", ".prs", ".slow", ".archive"];

fn seed_max_seq_entry(root: &Path, period: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::create_dir(root.join(format!("{period}_szza2_last-slot"))).unwrap();
}

#[test]
fn help_exits_zero_and_does_not_advertise_json_switch() {
    let output = bin_cmd()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.contains("--json"));
}

#[test]
fn json_switch_is_not_accepted() {
    assert_failure_stdout_empty(
        bin_cmd()
            .args(["--json", "new", "fix auth state", "--dry-run"])
            .assert(),
    );
}

#[test]
fn new_command_json_switch_is_not_accepted() {
    assert_failure_stdout_empty(
        bin_cmd()
            .args(["new", "--json", "fix auth state", "--dry-run"])
            .assert(),
    );
}

#[test]
fn new_command_help_does_not_advertise_json_switch() {
    let output = bin_cmd()
        .args(["new", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.contains("--json"));
}

#[test]
fn new_dry_run_returns_plan_json_by_default_without_creating_root() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    assert_json_allocation(&json, tmp.path(), "fix auth state", "fix-auth-state", true);

    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn new_creates_task_folder_and_prints_json_by_default() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let id = assert_json_allocation(&json, tmp.path(), "fix auth state", "fix-auth-state", false);
    assert!(tmp.path().join("stm").join(id).is_dir());
}

#[test]
fn new_uses_configured_active_root() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"work/tasks\"\n").unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let expected_root = tmp.path().canonicalize().unwrap().join("work/tasks");
    let id = assert_json_allocation_in_root(
        &json,
        &expected_root,
        "fix auth state",
        "fix-auth-state",
        false,
    );

    assert!(expected_root.join(id).is_dir());
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn new_scans_configured_roots_in_same_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nscan_roots = [\"stm/.pending\", \"stm/.archive\"]\n",
    )
    .unwrap();
    seed_max_seq_entry(&tmp.path().join("stm/.archive"), TEST_PERIOD);

    let output = bin_cmd()
        .args([
            "new",
            "fix auth state",
            "--dry-run",
            "--period",
            TEST_PERIOD,
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("monthly sequence exhausted"));
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("stm/.pending").exists());
}

#[test]
fn new_scans_each_default_scan_root_without_config() {
    // Each default scan root is decisive on its own: an entry seeded only
    // there must be enough to exhaust the namespace.
    for scan_root in DEFAULT_SCAN_ROOT_NAMES {
        let tmp = tempfile::tempdir().unwrap();
        seed_max_seq_entry(&tmp.path().join("stm").join(scan_root), TEST_PERIOD);

        let output = bin_cmd()
            .args([
                "new",
                "fix auth state",
                "--dry-run",
                "--period",
                TEST_PERIOD,
            ])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{scan_root}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("monthly sequence exhausted"),
            "{scan_root}: {stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn root_only_config_derives_each_scan_root_from_configured_root() {
    for scan_root in DEFAULT_SCAN_ROOT_NAMES {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"work/tasks\"\n").unwrap();
        seed_max_seq_entry(&tmp.path().join("work/tasks").join(scan_root), TEST_PERIOD);

        let output = bin_cmd()
            .args([
                "new",
                "fix auth state",
                "--dry-run",
                "--period",
                TEST_PERIOD,
            ])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{scan_root}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("monthly sequence exhausted"),
            "{scan_root}: {stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn new_treats_task_shaped_files_as_reservations_not_errors() {
    // A zipped task-folder export next to live folders participates in
    // sequencing and never breaks allocation.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("202605_szy2a_export-dump.zip"), "zip bytes").unwrap();

    let output = bin_cmd()
        .args(["new", "after the dump", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    // seq 658 ("zy") is reserved by the zip; the next allocation is 659.
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("szz"), "{sid_ref}");
}

#[test]
fn new_discovers_config_upward_and_resolves_paths_at_config_dir() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"tasks\"\n").unwrap();
    let nested = tmp.path().join("repo/deeply/nested");
    std::fs::create_dir_all(&nested).unwrap();

    let output = bin_cmd()
        .args(["new", "from below", "--period", "202605"])
        .current_dir(&nested)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let expected_root = tmp.path().canonicalize().unwrap().join("tasks");
    let id =
        assert_json_allocation_in_root(&json, &expected_root, "from below", "from-below", false);

    assert!(expected_root.join(&id).is_dir());
    assert!(!nested.join("tasks").exists());
    assert!(!nested.join("stm").exists());
}

#[test]
fn new_fails_closed_on_bad_parent_config_instead_of_walking_past_it() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"grand-tasks\"\n").unwrap();
    let mid = tmp.path().join("mid");
    std::fs::create_dir_all(mid.join("leaf")).unwrap();
    std::fs::write(mid.join(".sid"), "[tasks]\nroot = \"typo\"\n").unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run"])
        .current_dir(mid.join("leaf"))
        .output()
        .unwrap();

    assert!(!output.status.success(), "unexpected success");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("parse project config"), "{stderr}");
    assert!(stderr.contains("unknown field"), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("grand-tasks").exists());
}

#[cfg(unix)]
#[test]
fn new_fails_closed_on_dangling_parent_config_symlink() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join("missing-config"), tmp.path().join(".sid")).unwrap();
    let nested = tmp.path().join("nested");
    std::fs::create_dir_all(&nested).unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run"])
        .current_dir(&nested)
        .output()
        .unwrap();

    assert!(!output.status.success(), "unexpected success");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("read project config"), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(!nested.join("stm").exists());
}

#[test]
fn new_prefers_nearest_config_when_walking_upward() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"outer-tasks\"\n").unwrap();
    let inner = tmp.path().join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join(".sid"), "[task]\nroot = \"inner-tasks\"\n").unwrap();

    let output = bin_cmd()
        .args(["new", "nearest wins", "--period", "202605"])
        .current_dir(&inner)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let expected_root = inner.canonicalize().unwrap().join("inner-tasks");
    let id = assert_json_allocation_in_root(
        &json,
        &expected_root,
        "nearest wins",
        "nearest-wins",
        false,
    );

    assert!(expected_root.join(&id).is_dir());
    assert!(!tmp.path().join("outer-tasks").exists());
}

fn list_stdout(dir: &Path, args: &[&str]) -> Value {
    let output = bin_cmd()
        .arg("list")
        .args(args)
        .current_dir(dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    parse_stdout_json(&output)
}

#[test]
fn list_returns_empty_tasks_for_empty_project() {
    let tmp = tempfile::tempdir().unwrap();

    let json = list_stdout(tmp.path(), &[]);

    assert_json_keys(&json, &["tasks"]);
    assert_eq!(json["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn list_reports_recognized_entries_across_all_roots_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(stm.join(".pending")).unwrap();
    std::fs::create_dir_all(stm.join(".prs")).unwrap();
    std::fs::create_dir_all(stm.join(".slow")).unwrap();
    std::fs::create_dir_all(stm.join(".archive")).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_alpha")).unwrap();
    std::fs::create_dir(stm.join(".pending").join("202606_sde2a")).unwrap();
    std::fs::write(
        stm.join(".prs").join("202606_sdf2a_export-dump.zip"),
        "zip bytes",
    )
    .unwrap();
    std::fs::create_dir(stm.join(".slow").join("202606_sdg2a_slow-burn")).unwrap();
    std::fs::create_dir(stm.join(".archive").join("202605_szza2_old")).unwrap();
    // Ignored: non-recognized refs and non-task names.
    std::fs::create_dir(stm.join("202606_sdmr192_widened")).unwrap();
    std::fs::create_dir(stm.join(".archive").join("202605_81c1_legacy")).unwrap();
    std::fs::write(stm.join("README.md"), "").unwrap();

    let json = list_stdout(tmp.path(), &["--sort", "id"]);

    let tasks = json["tasks"].as_array().unwrap();
    let ids: Vec<&str> = tasks.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "202605_szza2_old",
            "202606_sdd2a_alpha",
            "202606_sde2a",
            "202606_sdf2a_export-dump.zip",
            "202606_sdg2a_slow-burn",
        ]
    );

    for task in tasks {
        assert_json_keys(
            task,
            &[
                "id", "modified", "path", "period", "root", "sid_ref", "slug",
            ],
        );
    }
    let canonical = tmp.path().canonicalize().unwrap().join("stm");
    let alpha = &tasks[1];
    assert_eq!(alpha["period"], "202606");
    assert_eq!(alpha["sid_ref"], "sdd2a");
    assert_eq!(alpha["slug"], "alpha");
    assert_eq!(PathBuf::from(alpha["root"].as_str().unwrap()), canonical);
    assert_eq!(
        PathBuf::from(alpha["path"].as_str().unwrap()),
        canonical.join("202606_sdd2a_alpha")
    );
    // Slugless reservation reports an empty slug; the zip keeps its tail.
    assert_eq!(tasks[2]["slug"], "");
    assert_eq!(tasks[3]["slug"], "export-dump.zip");
}

#[test]
fn list_filters_by_ref_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_alpha")).unwrap();
    std::fs::create_dir(stm.join("202606_sde2a_beta")).unwrap();
    std::fs::create_dir(stm.join("202605_szza2_old")).unwrap();

    let count = |args: &[&str]| {
        list_stdout(tmp.path(), args)["tasks"]
            .as_array()
            .unwrap()
            .len()
    };

    assert_eq!(count(&["sd"]), 2);
    assert_eq!(count(&["sdd2a"]), 1);
    assert_eq!(count(&["sz"]), 1);
    assert_eq!(count(&["szz9"]), 0);
}

fn set_mtime(path: &Path, when: std::time::SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

#[test]
fn list_defaults_to_most_recently_touched_first() {
    use std::time::{Duration, SystemTime};

    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    // Future child mtimes dominate the folders' own creation times, so the
    // recency order is fully controlled regardless of creation order. A
    // direct child edit must surface its folder (depth-1 touch proxy).
    let base = SystemTime::now();
    for (name, hours) in [
        ("202606_sdd2a_alpha", 1u64),
        ("202606_sde2a_beta", 3),
        ("202606_sdf2a_gamma", 2),
    ] {
        let dir = stm.join(name);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("PLAN.md"), "notes").unwrap();
        set_mtime(
            &dir.join("PLAN.md"),
            base + Duration::from_secs(hours * 3600),
        );
    }
    // A non-directory reservation sorts by its own mtime.
    std::fs::write(stm.join("202606_sdg2a_export.zip"), "zip").unwrap();
    set_mtime(
        &stm.join("202606_sdg2a_export.zip"),
        base + Duration::from_secs(4 * 3600),
    );

    let json = list_stdout(tmp.path(), &[]);

    let ids: Vec<&str> = json["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "202606_sdg2a_export.zip",
            "202606_sde2a_beta",
            "202606_sdf2a_gamma",
            "202606_sdd2a_alpha",
        ]
    );
}

#[test]
fn list_filters_by_slug_words_and_ref_prefix_terms() {
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_historical-delivery-triage")).unwrap();
    std::fs::create_dir(stm.join("202606_sde2a_java-review")).unwrap();
    std::fs::create_dir(stm.join("202605_szza2_delivery-archive")).unwrap();

    let count = |args: &[&str]| {
        list_stdout(tmp.path(), args)["tasks"]
            .as_array()
            .unwrap()
            .len()
    };

    // Single word, multiple matches.
    assert_eq!(count(&["delivery"]), 2);
    // Terms AND together.
    assert_eq!(count(&["delivery", "triage"]), 1);
    assert_eq!(count(&["delivery", "review"]), 0);
    // Case-insensitive, on both the slug and ref-prefix arms.
    assert_eq!(count(&["DELIVERY", "Triage"]), 1);
    assert_eq!(count(&["SD"]), count(&["sd"]));
    // A term can mix with a ref prefix.
    assert_eq!(count(&["sz", "delivery"]), 1);
    // Partial word.
    assert_eq!(count(&["deliv"]), 2);
}

#[test]
fn list_human_mode_prints_aligned_plain_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(stm.join(".slow")).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_alpha")).unwrap();
    std::fs::create_dir(stm.join(".slow").join("202606_sde2a_slow-burn")).unwrap();

    let output = bin_cmd()
        .args(["list", "--human", "--sort", "id"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.contains('{'), "{stdout}");

    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("202606_sdd2a_alpha"), "{stdout}");
    assert!(lines[1].contains("202606_sde2a_slow-burn"), "{stdout}");
    // Project-relative root column.
    assert!(lines[0].trim_end().ends_with("stm"), "{stdout}");
    assert!(lines[1].trim_end().ends_with("stm/.slow"), "{stdout}");
    // Local-time prefix, RFC3339 'T' replaced for human reading.
    assert!(lines[0].starts_with("20"), "{stdout}");
    assert!(
        !lines[0].split("  ").next().unwrap().contains('T'),
        "{stdout}"
    );
}

#[test]
fn list_human_mode_with_no_matches_prints_nothing() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .args(["list", "--human", "nope"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(output.is_empty());
}

#[test]
fn list_default_output_stays_json_with_exact_keys() {
    // The machine contract is flag-free JSON regardless of --human existing.
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_alpha")).unwrap();

    let json = list_stdout(tmp.path(), &[]);
    assert_json_keys(&json, &["tasks"]);
    assert_json_keys(
        &json["tasks"].as_array().unwrap()[0],
        &[
            "id", "modified", "path", "period", "root", "sid_ref", "slug",
        ],
    );
}

#[test]
fn list_fails_closed_on_unreadable_scan_root() {
    // Both output modes fail closed identically: empty stdout, stderr
    // diagnostic, exit 1.
    for args in [&[][..], &["--human"][..]] {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("stm")).unwrap();
        std::fs::write(tmp.path().join("stm/.pending"), "not a dir").unwrap();

        let output = bin_cmd()
            .arg("list")
            .args(args)
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{args:?}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("scan task root"), "{args:?}: {stderr}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn list_human_mode_shows_base_relative_roots_from_subdirectories() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"tasks\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("tasks/.slow")).unwrap();
    std::fs::create_dir(tmp.path().join("tasks/.slow/202606_sdd2a_alpha")).unwrap();
    let nested = tmp.path().join("repo/nested");
    std::fs::create_dir_all(&nested).unwrap();

    let output = bin_cmd()
        .args(["list", "--human"])
        .current_dir(&nested)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Relative to the project base (the .sid directory), not to cwd.
    assert!(stdout.trim_end().ends_with("tasks/.slow"), "{stdout}");
}

#[test]
fn list_discovers_config_upward() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"tasks\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("tasks")).unwrap();
    std::fs::create_dir(tmp.path().join("tasks/202606_sdd2a_alpha")).unwrap();
    let nested = tmp.path().join("repo/nested");
    std::fs::create_dir_all(&nested).unwrap();

    let json = list_stdout(&nested, &[]);

    let tasks = json["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "202606_sdd2a_alpha");
}

#[test]
fn new_into_allocates_directly_into_configured_roots() {
    // Shorthand and full forms both resolve; the active root is itself a
    // valid (no-op) destination since it is part of the configured list.
    for (into, expected_rel) in [
        (".slow", "stm/.slow"),
        ("./.slow", "stm/.slow"),
        ("stm/.slow", "stm/.slow"),
        ("stm", "stm"),
    ] {
        let tmp = tempfile::tempdir().unwrap();

        let output = bin_cmd()
            .args(["new", "slow burn", "--into", into, "--period", "202605"])
            .current_dir(tmp.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_stdout_json(&output);
        let expected_root = tmp.path().canonicalize().unwrap().join(expected_rel);
        let id =
            assert_json_allocation_in_root(&json, &expected_root, "slow burn", "slow-burn", false);

        assert!(expected_root.join(&id).is_dir(), "{into}");
    }
}

#[test]
fn new_into_dry_run_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .args([
            "new",
            "slow burn",
            "--into",
            ".slow",
            "--dry-run",
            "--period",
            "202605",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    assert_eq!(json["dry_run"], true);
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn new_into_shares_the_namespace_with_other_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    // seq 658 in the active root; the .slow allocation must take 659.
    std::fs::create_dir(stm.join("202605_szy2a_penultimate")).unwrap();

    let output = bin_cmd()
        .args(["new", "final slot", "--into", ".slow", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("szz"), "{sid_ref}");
    assert!(
        tmp.path()
            .join("stm/.slow")
            .join(json["id"].as_str().unwrap())
            .is_dir()
    );
}

#[test]
fn new_into_rejects_unconfigured_destinations() {
    for into in [".nope", "slow", "", "."] {
        let tmp = tempfile::tempdir().unwrap();

        let output = bin_cmd()
            .args(["new", "slow burn", "--into", into, "--dry-run"])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{into}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("not a configured task root"),
            "{into}: {stderr}"
        );
        assert!(output.stdout.is_empty());
        assert!(!tmp.path().join("stm").exists(), "{into}");
    }
}

#[test]
fn duplicate_configured_roots_are_deduplicated() {
    // scan_roots means *additional* roots, but repeating the active root is a
    // plausible hand-written config; it must not double list output or make
    // --into ambiguous against itself.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"stm\"\nscan_roots = [\"stm\", \"stm/.archive\", \"stm/.archive\"]\n",
    )
    .unwrap();
    let stm = tmp.path().join("stm");
    std::fs::create_dir_all(&stm).unwrap();
    std::fs::create_dir(stm.join("202606_sdd2a_alpha")).unwrap();

    let json = list_stdout(tmp.path(), &[]);
    assert_eq!(json["tasks"].as_array().unwrap().len(), 1);

    bin_cmd()
        .args([
            "new",
            "fix auth state",
            "--into",
            "stm",
            "--dry-run",
            "--period",
            "202605",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

#[test]
fn new_into_rejects_ambiguous_destinations() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nscan_roots = [\"alpha/.x\", \"beta/.x\"]\n",
    )
    .unwrap();

    let output = bin_cmd()
        .args(["new", "slow burn", "--into", ".x", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!output.status.success(), "unexpected success");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("ambiguous"), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn new_period_override_uses_deterministic_start_for_empty_month() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    assert_eq!(json["period"], "202605");
    // deterministic_seq_start("202605") == 128, which encodes as "ea".
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("sea"), "{sid_ref}");
}

#[test]
fn new_period_override_allocates_final_sequence_then_exhausts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    std::fs::create_dir_all(&root).unwrap();
    // seq 658 encodes as "zy"; the next allocation takes the final slot 659.
    std::fs::create_dir(root.join("202605_szy2a_penultimate")).unwrap();

    let output = bin_cmd()
        .args(["new", "final slot", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("szz"), "{sid_ref}");

    let output = bin_cmd()
        .args(["new", "one too many", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("monthly sequence exhausted"));
    assert!(output.stdout.is_empty());
}

#[test]
fn new_rejects_malformed_period_override() {
    // Both sides of the six-digit boundary (5 and 7), plus shape violations.
    for period in ["20261", "2026056", "2026", "2026-5", "abcdef"] {
        let tmp = tempfile::tempdir().unwrap();

        let output = bin_cmd()
            .args(["new", "fix auth state", "--dry-run", "--period", period])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{period}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("invalid period"), "{period}: {stderr}");
        assert!(output.stdout.is_empty());
        assert!(!tmp.path().join("stm").exists());
    }
}

#[test]
fn new_command_help_does_not_advertise_period_seam() {
    let output = bin_cmd()
        .args(["new", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.contains("--period"));
}

#[cfg(unix)]
#[test]
fn new_follows_symlinked_scan_roots() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nscan_roots = [\"linked-archive\"]\n",
    )
    .unwrap();
    seed_max_seq_entry(&tmp.path().join("real-archive"), TEST_PERIOD);
    symlink(
        tmp.path().join("real-archive"),
        tmp.path().join("linked-archive"),
    )
    .unwrap();

    let output = bin_cmd()
        .args([
            "new",
            "fix auth state",
            "--dry-run",
            "--period",
            TEST_PERIOD,
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("monthly sequence exhausted"));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn new_treats_dangling_symlinked_scan_root_as_empty() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nscan_roots = [\"linked-archive\"]\n",
    )
    .unwrap();
    symlink(
        tmp.path().join("missing-archive"),
        tmp.path().join("linked-archive"),
    )
    .unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    // The dangling root reads as an empty snapshot: deterministic start.
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("sea"), "{sid_ref}");
}

#[cfg(unix)]
#[test]
fn new_follows_symlinked_active_root_for_scan_and_create() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"linked-root\"\n").unwrap();
    seed_max_seq_entry(&tmp.path().join("real-root"), TEST_PERIOD);
    symlink(tmp.path().join("real-root"), tmp.path().join("linked-root")).unwrap();

    // Scanning follows the link: the seeded entry exhausts its period.
    let output = bin_cmd()
        .args([
            "new",
            "fix auth state",
            "--dry-run",
            "--period",
            TEST_PERIOD,
        ])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("monthly sequence exhausted"), "{stderr}");

    // Creation goes through the link into the real directory.
    let output = bin_cmd()
        .args(["new", "fix auth state", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let id = json["id"].as_str().unwrap();
    assert!(tmp.path().join("real-root").join(id).is_dir());
}

#[cfg(unix)]
#[test]
fn new_dangling_symlinked_active_root_scans_empty_but_fails_create() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "[task]\nroot = \"linked-root\"\n").unwrap();
    symlink(
        tmp.path().join("missing-root"),
        tmp.path().join("linked-root"),
    )
    .unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run", "--period", "202605"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let sid_ref = json["sid_ref"].as_str().unwrap();
    assert!(sid_ref.starts_with("sea"), "{sid_ref}");

    // A real run cannot materialize the root through the dangling link.
    let output = bin_cmd()
        .args(["new", "fix auth state", "--period", "202605"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("create task root"), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("missing-root").exists());
}

#[test]
fn new_skips_missing_configured_scan_roots_and_creates_only_active_root() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"active\"\nscan_roots = [\"missing-pending\", \"missing-archive\"]\n",
    )
    .unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    let expected_root = tmp.path().canonicalize().unwrap().join("active");
    let id = assert_json_allocation_in_root(
        &json,
        &expected_root,
        "fix auth state",
        "fix-auth-state",
        false,
    );

    assert!(expected_root.join(id).is_dir());
    assert!(!tmp.path().join("missing-pending").exists());
    assert!(!tmp.path().join("missing-archive").exists());
}

#[test]
fn new_fails_closed_when_configured_scan_root_cannot_be_read() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("not-a-dir"), "").unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nscan_roots = [\"not-a-dir\"]\n",
    )
    .unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("scan task root"));
    assert!(stderr.contains("not-a-dir"));
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn new_rejects_unknown_config_keys() {
    for config in [
        "[tasks]\nroot = \"active\"\n",
        "[task]\nscan_root = [\"stm/.archive\"]\n",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".sid"), config).unwrap();

        let output = bin_cmd()
            .args(["new", "fix auth state", "--dry-run"])
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();

        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("parse project config"));
        assert!(stderr.contains("unknown field"));
        assert!(output.stdout.is_empty());
        assert!(!tmp.path().join("stm").exists());
    }
}

#[test]
fn new_rejects_absolute_config_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let absolute_root = tmp.path().join("outside-root");
    let absolute_scan_root = tmp.path().join("outside-scan");

    for config in [
        format!("[task]\nroot = \"{}\"\n", absolute_root.display()),
        format!(
            "[task]\nscan_roots = [\"{}\"]\n",
            absolute_scan_root.display()
        ),
    ] {
        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join(".sid"), config).unwrap();

        let output = bin_cmd()
            .args(["new", "fix auth state", "--dry-run"])
            .current_dir(project.path())
            .assert()
            .failure()
            .get_output()
            .clone();

        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("project config paths must be relative"));
        assert!(output.stdout.is_empty());
        assert!(!absolute_root.exists());
        assert!(!absolute_scan_root.exists());
    }
}

#[test]
fn new_rejects_empty_and_curdir_config_paths() {
    for config in [
        "[task]\nroot = \"\"\n",
        "[task]\nroot = \".\"\n",
        "[task]\nscan_roots = [\"\"]\n",
        "[task]\nscan_roots = [\".\"]\n",
    ] {
        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join(".sid"), config).unwrap();

        let output = bin_cmd()
            .args(["new", "fix auth state", "--dry-run"])
            .current_dir(project.path())
            .output()
            .unwrap();

        assert!(!output.status.success(), "{config}: unexpected success");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("must name a directory"),
            "{config}: {stderr}"
        );
        assert!(output.stdout.is_empty());
        assert!(!project.path().join("stm").exists());
    }
}

#[test]
fn new_rejects_parent_dir_config_paths() {
    for config in [
        "[task]\nroot = \"../outside-root\"\n",
        "[task]\nscan_roots = [\"stm/../outside-scan\"]\n",
    ] {
        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join(".sid"), config).unwrap();

        let output = bin_cmd()
            .args(["new", "fix auth state", "--dry-run"])
            .current_dir(project.path())
            .assert()
            .failure()
            .get_output()
            .clone();

        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("project config paths may not contain"));
        assert!(output.stdout.is_empty());
        assert!(!project.path().join("../outside-root").exists());
        assert!(!project.path().join("outside-scan").exists());
    }
}

#[cfg(unix)]
#[test]
fn new_fails_closed_on_dangling_config_symlink() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join("missing-config"), tmp.path().join(".sid")).unwrap();

    let output = bin_cmd()
        .args(["new", "fix auth state", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("read project config"));
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn init_writes_default_config_to_cwd() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    assert_json_keys(&json, &["created", "path"]);
    assert_eq!(json["created"], true);
    assert_eq!(
        PathBuf::from(json["path"].as_str().unwrap()),
        tmp.path().canonicalize().unwrap().join(".sid")
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".sid")).unwrap(),
        "[task]\nroot = \"stm\"\nscan_roots = [\"stm/.pending\", \"stm/.prs\", \"stm/.slow\", \"stm/.archive\"]\n\n[seed]\nroot = \"stm/.seeds\"\n\n[note]\nroot = \"stm/.notes\"\n\n[topic]\nroots = []\n\n[queue]\nstale_after_days = 7\n"
    );
}

#[test]
fn typed_config_paths_validate_and_unknown_keys_fail_closed() {
    for config in [
        "[task]\nroot = \"stm\"\n[seed]\nroot = \"../seeds\"\n",
        "[task]\nroot = \"stm\"\n[note]\nroot = \"/notes\"\n",
        "[task]\nroot = \"stm\"\n[topic]\nroots = [\".\"]\n",
        "[task]\nroot = \"stm\"\n[queue]\ndays = 7\n",
        "[task]\nroot = \"stm\"\n[seed]\nroots = []\n",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".sid"), config).unwrap();
        let output = bin_cmd()
            .args(["new", "typed config", "--dry-run"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(!output.status.success(), "unexpected success for {config}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn seed_root_reserves_allocation_refs_but_is_not_listed_or_an_into_target() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"tasks\"\nscan_roots = []\n[seed]\nroot = \"captures/seeds\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("captures/seeds")).unwrap();
    std::fs::write(
        tmp.path().join("captures/seeds/202606_szza2_parked.md"),
        "---\ntype: \"seed\"\nid: \"szza2\"\ntitle: \"Parked\"\ntimestamp: \"2026-06-01\"\n---\n",
    )
    .unwrap();

    let list = bin_cmd()
        .arg("list")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&list)["tasks"], serde_json::json!([]));

    assert_failure_stdout_empty(
        bin_cmd()
            .args(["new", "blocked", "--period", TEST_PERIOD, "--dry-run"])
            .current_dir(tmp.path())
            .assert(),
    );
    assert_failure_stdout_empty(
        bin_cmd()
            .args(["new", "blocked", "--into", "captures/seeds", "--dry-run"])
            .current_dir(tmp.path())
            .assert(),
    );
}

#[cfg(unix)]
#[test]
fn init_reports_dangling_config_symlink_distinctly() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join("missing-config"), tmp.path().join(".sid")).unwrap();

    let output = bin_cmd()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("dangling symlink"), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(!tmp.path().join("missing-config").exists());
}

#[cfg(unix)]
#[test]
fn init_does_not_misreport_resolvable_or_looping_config_symlinks_as_dangling() {
    use std::os::unix::fs::symlink;

    // Symlink to an existing config: "already exists", target untouched.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("real-config"), "# keep me\n").unwrap();
    symlink(tmp.path().join("real-config"), tmp.path().join(".sid")).unwrap();

    let output = bin_cmd()
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("project config already exists"), "{stderr}");
    assert!(!stderr.contains("dangling symlink"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("real-config")).unwrap(),
        "# keep me\n"
    );

    // Symlink loop: unresolvable but not dangling; keep the conservative
    // message.
    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join(".sid"), tmp.path().join(".sid")).unwrap();

    let output = bin_cmd()
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("project config already exists"), "{stderr}");
    assert!(!stderr.contains("dangling symlink"), "{stderr}");
}

#[test]
fn init_refuses_to_overwrite_existing_config() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".sid"), "# keep me\n").unwrap();

    let output = bin_cmd()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("project config already exists"));
    assert!(output.stdout.is_empty());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".sid")).unwrap(),
        "# keep me\n"
    );
}

#[test]
fn agent_instructions_returns_json_envelope_by_default() {
    let tmp = tempfile::tempdir().unwrap();

    let output = bin_cmd()
        .arg("agent-instructions")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_stdout_json(&output);
    assert_json_keys(&json, &["format", "text"]);
    assert_eq!(json["format"], "markdown");
    assert!(json["text"].as_str().unwrap().contains("sid new"));
}

#[test]
fn new_rejects_empty_slug_titles_with_clear_error() {
    for title in ["...", "   \t  ", "東京🙂"] {
        let tmp = tempfile::tempdir().unwrap();

        let output = bin_cmd()
            .args(["new", title])
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();

        let stderr = String::from_utf8(output.stderr.clone()).unwrap();
        assert!(stderr.contains("task title must contain"));
        assert!(output.stdout.is_empty());
        assert!(!tmp.path().join("stm").exists());
    }
}
