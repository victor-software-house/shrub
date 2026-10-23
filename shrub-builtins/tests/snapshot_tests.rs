//! Integration tests for `Shell::snapshot` and `Shell::restore_snapshot`.
//!
//! The state is set up by running commands, so these tests live beside the
//! builtins that set it; each check restores a snapshot into a fresh shell,
//! with no command run in it first, and compares what the builtins print in
//! both shells.

#![cfg(unix)]
#![cfg(test)]
#![allow(clippy::panic_in_result_fn, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use shrub_builtins::{BuiltinSet, ShellBuilderExt as _};
use shrub_core::env::EnvironmentScope;
use shrub_core::snapshot::{
    NotKept, NotKeptKind, RestoreFailure, RestoreFailureReason, SavedValue, SavedVariable,
    ShellSnapshot, SnapshotOptions, StateKind,
};
use shrub_core::variables::{ShellValueUnsetType, ShellVariableUpdateTransform};
use shrub_core::{ProfileLoadBehavior, RcLoadBehavior, Shell, ShellValue, ShellVariable};

async fn new_shell(dir: &Path) -> Result<Shell> {
    Ok(Shell::builder()
        .default_builtins(BuiltinSet::BashMode)
        .do_not_inherit_env(true)
        .profile(ProfileLoadBehavior::Skip)
        .rc(RcLoadBehavior::Skip)
        .kill_external_commands_on_drop(true)
        .working_dir(dir.to_path_buf())
        .build()
        .await?)
}

async fn run(shell: &mut Shell, script: &str) -> Result<u8> {
    let params = shell.default_exec_params();
    let result = shell
        .run_string(script, &shrub_core::SourceInfo::default(), &params)
        .await?;
    Ok(u8::from(result.exit_code))
}

/// Runs `script` with its output in a file, and returns that output.
async fn output(shell: &mut Shell, script: &str) -> Result<String> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("out");
    shell.set_env_global(
        "OUT",
        ShellVariable::new(path.to_string_lossy().into_owned()),
    )?;
    run(shell, &format!("{{\n{script}\n}} >\"$OUT\" 2>&1")).await?;
    Ok(std::fs::read_to_string(path)?)
}

/// Takes a snapshot of `shell` and restores it into a fresh shell, which it
/// returns with the snapshot; the restore must apply every item.
async fn restored(shell: &Shell, dir: &Path) -> Result<(Shell, ShellSnapshot)> {
    let snapshot = shell.snapshot(&SnapshotOptions::default())?;
    let mut fresh = new_shell(dir).await?;
    let report = fresh.restore_snapshot(snapshot.clone())?;
    assert_eq!(report.failed, Vec::<RestoreFailure>::new());
    Ok((fresh, snapshot))
}

const SETUP: &str = r#"
cd sub
export A=1
B='single '"'"' double " dollar $HOME backquote ` backslash \'
CTRL=$'tab\there\nnew line\x01'
declare -i N=6*7
declare -r R=fixed
declare -n REF=A
declare -l LOW=MiXeD
declare -u UP=MiXeD
declare -A MAP=([one]=1 ['two words']='a "b" $c' ['x=y; z']=z)
ARR=([0]=x [5]='y z')
declare -a EMPTY
export LATER
count() {
    local item
    for item in "$@"; do
        if [ "$item" = skip ]; then
            continue
        fi
        echo "<$item>"
    done
}
export -f count
greet() {
    cat <<EOF
hello $1
EOF
    echo "after heredoc"
}
export -f greet
alias ll='ls -l'
set -o noclobber
shopt -s nullglob
trap 'echo caught' USR1
pushd "$PWD" >/dev/null
cd ..
set -- first 'second arg'
enable -n printf
false
"#;

const PROBE: &str = r"
echo status $?
declare -p A B CTRL N R REF LOW UP MAP ARR EMPTY LATER
declare -f
alias -p
set +o
shopt -p
trap -p
dirs -l -p
pwd
echo $# $*
enable -n
count a skip 'b c'
";

#[tokio::test]
async fn a_restored_shell_prints_the_same_state() -> Result<()> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir(dir.path().join("sub"))?;
    let mut shell = new_shell(dir.path()).await?;
    assert_eq!(run(&mut shell, SETUP).await?, 1);

    let (mut fresh, snapshot) = restored(&shell, dir.path()).await?;
    // shrub prints a here-document's body right after its operator, so the
    // source of a function holding one does not parse back; the snapshot
    // names it instead of keeping it.
    assert_eq!(
        snapshot.not_kept,
        vec![NotKept {
            kind: NotKeptKind::Function,
            name: "greet".to_owned(),
        }]
    );
    assert!(snapshot.functions.contains_key("count"));
    assert_eq!(
        fresh.snapshot(&SnapshotOptions::default())?,
        ShellSnapshot {
            not_kept: vec![],
            ..snapshot
        }
    );

    // Through the API, so no command resets `$?` in the original shell.
    assert!(shell.undefine_func("greet"));
    let before = output(&mut shell, PROBE).await?;
    let after = output(&mut fresh, PROBE).await?;
    assert_eq!(after, before);
    assert!(before.starts_with("status 1\n"), "{before}");
    assert!(before.ends_with("<a>\n<b c>\n"), "{before}");
    Ok(())
}

#[tokio::test]
async fn nothing_in_a_snapshot_runs_as_a_command() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let canary = dir.path().join("canary");
    let touch = format!("touch \"{}\"", canary.display());
    let mut shell = new_shell(dir.path()).await?;

    shell.set_env_global("V", ShellVariable::new(format!("$({touch})")))?;
    shell
        .aliases_mut()
        .insert(format!("x$({touch})"), touch.clone());
    shell
        .aliases_mut()
        .insert("a;b\nc".to_owned(), touch.clone());
    run(
        &mut shell,
        &format!("trap '{touch}' USR1; f() {{ {touch}; }}"),
    )
    .await?;

    let (fresh, _) = restored(&shell, dir.path()).await?;
    assert!(!canary.exists());
    assert_eq!(
        fresh.env_str("V").as_deref(),
        Some(format!("$({touch})").as_str())
    );
    assert_eq!(fresh.aliases(), shell.aliases());
    Ok(())
}

#[tokio::test]
async fn a_job_and_an_open_descriptor_are_named_as_not_kept() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut shell = new_shell(dir.path()).await?;
    run(&mut shell, "sleep 30 & exec 3>/dev/null").await?;

    let snapshot = shell.snapshot(&SnapshotOptions::default())?;
    run(&mut shell, "kill %1; exec 3>&-").await?;

    assert_eq!(
        snapshot.not_kept,
        vec![
            NotKept {
                kind: NotKeptKind::Job,
                name: "%1 sleep".to_owned(),
            },
            NotKept {
                kind: NotKeptKind::OpenFile,
                name: "3".to_owned(),
            },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn the_baseline_and_excluded_names_stay_out() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut shell = new_shell(dir.path()).await?;
    let mut exported = |name: &str, value: &str| -> Result<()> {
        let mut var = ShellVariable::new(value);
        var.export();
        Ok(shell.set_env_global(name, var)?)
    };
    exported("SAME", "base")?;
    exported("CHANGED", "new")?;
    exported("SECRET", "token")?;
    let options = SnapshotOptions {
        baseline: BTreeMap::from([
            ("SAME".to_owned(), "base".to_owned()),
            ("CHANGED".to_owned(), "base".to_owned()),
            ("GONE".to_owned(), "base".to_owned()),
            ("SECRET".to_owned(), "token".to_owned()),
        ]),
        exclude: BTreeSet::from(["SECRET".to_owned()]),
    };

    let snapshot = shell.snapshot(&options)?;
    assert!(!snapshot.variables.contains_key("SAME"));
    assert!(snapshot.variables.contains_key("CHANGED"));
    assert!(!snapshot.variables.contains_key("SECRET"));
    assert_eq!(
        snapshot.unset_variables,
        BTreeSet::from(["GONE".to_owned()])
    );
    for name in ["PWD", "OLDPWD", "_", "BASHPID", "PPID", "UID", "SHLVL"] {
        assert!(!snapshot.variables.contains_key(name), "{name}");
    }

    let mut fresh = new_shell(dir.path()).await?;
    fresh.set_env_global("GONE", ShellVariable::new("base"))?;
    fresh.restore_snapshot(snapshot)?;
    assert_eq!(fresh.env_str("GONE"), None);
    assert_eq!(fresh.env_str("CHANGED").as_deref(), Some("new"));
    Ok(())
}

#[tokio::test]
async fn restore_replaces_functions_aliases_and_traps() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let shell = new_shell(dir.path()).await?;
    let snapshot = shell.snapshot(&SnapshotOptions::default())?;

    let mut fresh = new_shell(dir.path()).await?;
    run(
        &mut fresh,
        "f() { :; }; alias x=y; trap 'echo t' USR1; enable -n printf",
    )
    .await?;
    fresh.restore_snapshot(snapshot)?;

    assert_eq!(
        output(&mut fresh, "declare -f; alias -p; trap -p; enable -n").await?,
        ""
    );
    Ok(())
}

#[tokio::test]
async fn an_item_that_cannot_apply_is_reported_and_the_rest_apply() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let shell = new_shell(dir.path()).await?;
    let mut snapshot = shell.snapshot(&SnapshotOptions::default())?;
    snapshot.working_dir = dir.path().join("gone");
    snapshot.variables.insert(
        "R".to_owned(),
        SavedVariable {
            value: SavedValue::String("other".to_owned()),
            exported: false,
            readonly: true,
            integer: false,
            nameref: false,
            trace: false,
            transform: ShellVariableUpdateTransform::None,
        },
    );
    snapshot.variables.insert(
        "KEPT".to_owned(),
        SavedVariable {
            value: SavedValue::String("yes".to_owned()),
            exported: false,
            readonly: false,
            integer: false,
            nameref: false,
            trace: false,
            transform: ShellVariableUpdateTransform::None,
        },
    );
    snapshot.functions.insert(
        "broken".to_owned(),
        shrub_core::snapshot::SavedFunction {
            body: "() { ".to_owned(),
            exported: false,
        },
    );
    snapshot
        .traps
        .insert("NOSUCHSIGNAL".to_owned(), "echo".to_owned());
    snapshot.set_options.insert("nosuchoption".to_owned(), true);
    snapshot
        .disabled_builtins
        .insert("nosuchbuiltin".to_owned());

    let mut fresh = new_shell(dir.path()).await?;
    run(&mut fresh, "declare -r R=fixed").await?;
    let report = fresh.restore_snapshot(snapshot)?;

    let failed: Vec<_> = report
        .failed
        .iter()
        .map(|failure| (failure.kind, failure.name.as_str(), failure.reason))
        .collect();
    let gone = dir.path().join("gone");
    assert_eq!(
        failed,
        vec![
            (
                StateKind::WorkingDir,
                gone.to_string_lossy().as_ref(),
                RestoreFailureReason::NotADirectory
            ),
            (StateKind::Variable, "R", RestoreFailureReason::Readonly),
            (
                StateKind::SetOption,
                "nosuchoption",
                RestoreFailureReason::UnknownOption
            ),
            (
                StateKind::Function,
                "broken",
                RestoreFailureReason::DoesNotParse
            ),
            (
                StateKind::Trap,
                "NOSUCHSIGNAL",
                RestoreFailureReason::UnknownSignal
            ),
            (
                StateKind::DisabledBuiltin,
                "nosuchbuiltin",
                RestoreFailureReason::UnknownBuiltin
            ),
        ]
    );
    assert_eq!(fresh.working_dir(), dir.path());
    assert_eq!(fresh.env_str("R").as_deref(), Some("fixed"));
    assert_eq!(fresh.env_str("KEPT").as_deref(), Some("yes"));
    Ok(())
}

#[tokio::test]
async fn functions_parse_under_the_restored_options() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut shell = new_shell(dir.path()).await?;
    run(&mut shell, "shopt -s extglob; f() { echo @(a|b); }").await?;

    let snapshot = shell.snapshot(&SnapshotOptions::default())?;
    assert_eq!(snapshot.shopt_options.get("extglob"), Some(&true));
    let mut fresh = new_shell(dir.path()).await?;
    run(&mut fresh, "shopt -u extglob").await?;
    let report = fresh.restore_snapshot(snapshot)?;

    assert_eq!(report.failed, Vec::<RestoreFailure>::new());
    assert_eq!(
        output(&mut fresh, "declare -f f").await?,
        output(&mut shell, "declare -f f").await?
    );
    Ok(())
}

#[tokio::test]
async fn a_readonly_pwd_is_reported_as_readonly() -> Result<()> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir(dir.path().join("sub"))?;
    let mut shell = new_shell(dir.path()).await?;
    run(&mut shell, "cd sub").await?;
    let snapshot = shell.snapshot(&SnapshotOptions::default())?;

    let mut fresh = new_shell(dir.path()).await?;
    run(&mut fresh, "readonly PWD").await?;
    let report = fresh.restore_snapshot(snapshot)?;

    let failed: Vec<_> = report
        .failed
        .iter()
        .map(|failure| (failure.kind, failure.reason))
        .collect();
    assert_eq!(
        failed,
        vec![(StateKind::WorkingDir, RestoreFailureReason::Readonly)]
    );
    assert_eq!(fresh.working_dir(), dir.path());
    Ok(())
}

#[tokio::test]
async fn a_snapshot_is_taken_and_restored_only_between_commands() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut shell = new_shell(dir.path()).await?;
    let snapshot = shell.snapshot(&SnapshotOptions::default())?;

    shell.env_mut().push_scope(EnvironmentScope::Local);
    let taken = shell.snapshot(&SnapshotOptions::default());
    let restored = shell.restore_snapshot(snapshot);
    shell.env_mut().pop_scope(EnvironmentScope::Local)?;

    for result in [taken.map(|_| ()), restored.map(|_| ())] {
        let error = result
            .err()
            .ok_or_else(|| anyhow!("no error inside a scope"))?;
        assert!(matches!(
            error.kind(),
            shrub_core::ErrorKind::NotBetweenCommands
        ));
    }
    Ok(())
}

/// A variable as the property test draws it.
#[derive(Clone, Debug)]
struct Drawn {
    value: SavedValue,
    exported: bool,
    readonly: bool,
    integer: bool,
    nameref: bool,
    trace: bool,
    transform: ShellVariableUpdateTransform,
}

/// Text weighted towards what quoting gets wrong, then anything at all.
fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[$`\\\\\"' \n\t\x01\x1b\x7f=\\[\\]é漢]{0,10}",
        any::<String>(),
    ]
}

fn drawn() -> impl Strategy<Value = Drawn> {
    let value = prop_oneof![
        prop_oneof![
            Just(ShellValueUnsetType::Untyped),
            Just(ShellValueUnsetType::IndexedArray),
            Just(ShellValueUnsetType::AssociativeArray),
        ]
        .prop_map(SavedValue::Unset),
        text().prop_map(SavedValue::String),
        prop::collection::btree_map(any::<u64>(), text(), 0..4).prop_map(SavedValue::IndexedArray),
        prop::collection::btree_map(text(), text(), 0..4).prop_map(SavedValue::AssociativeArray),
    ];
    let transform = prop_oneof![
        Just(ShellVariableUpdateTransform::None),
        Just(ShellVariableUpdateTransform::Lowercase),
        Just(ShellVariableUpdateTransform::Uppercase),
        Just(ShellVariableUpdateTransform::Capitalize),
    ];
    (value, any::<[bool; 5]>(), transform).prop_map(
        |(value, [exported, readonly, integer, nameref, trace], transform)| Drawn {
            value,
            exported,
            readonly,
            integer,
            nameref,
            trace,
            transform,
        },
    )
}

/// Sets `drawn` in `shell` through the variable API.
fn set(shell: &mut Shell, name: &str, drawn: &Drawn) -> Result<()> {
    let mut var = ShellVariable::new(match drawn.value.clone() {
        SavedValue::Unset(unset_type) => ShellValue::Unset(unset_type),
        SavedValue::String(value) => ShellValue::String(value),
        SavedValue::IndexedArray(values) => ShellValue::IndexedArray(values),
        SavedValue::AssociativeArray(values) => ShellValue::AssociativeArray(values),
    });
    if drawn.exported {
        var.export();
    }
    if drawn.readonly {
        var.set_readonly();
    }
    if drawn.integer {
        var.treat_as_integer();
    }
    if drawn.nameref {
        var.treat_as_nameref();
    }
    if drawn.trace {
        var.enable_trace();
    }
    var.set_update_transform(drawn.transform);
    Ok(shell.set_env_global(name, var)?)
}

async fn variables_round_trip(variables: BTreeMap<String, Drawn>) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut shell = new_shell(dir.path()).await?;
    for (name, drawn) in &variables {
        set(&mut shell, name, drawn)?;
    }

    let (mut fresh, snapshot) = restored(&shell, dir.path()).await?;
    assert_eq!(snapshot.not_kept, Vec::new());
    for (name, drawn) in &variables {
        let expected = SavedVariable {
            value: drawn.value.clone(),
            exported: drawn.exported,
            readonly: drawn.readonly,
            integer: drawn.integer,
            nameref: drawn.nameref,
            trace: drawn.trace,
            transform: drawn.transform,
        };
        assert_eq!(snapshot.variables.get(name), Some(&expected), "{name}");
    }
    assert_eq!(fresh.snapshot(&SnapshotOptions::default())?, snapshot);

    let names: Vec<_> = variables.keys().map(String::as_str).collect();
    if !names.is_empty() {
        let probe = format!("declare -p {}", names.join(" "));
        assert_eq!(
            output(&mut fresh, &probe).await?,
            output(&mut shell, &probe).await?
        );
    }
    Ok(())
}

#[test]
fn any_variables_round_trip() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut runner = TestRunner::new(Config {
        cases: 64,
        ..Config::default()
    });
    let variables = prop::collection::btree_map("v_[a-z0-9_]{0,8}", drawn(), 0..16);
    runner
        .run(&variables, |variables| {
            runtime
                .block_on(variables_round_trip(variables))
                .map_err(|error| TestCaseError::fail(error.to_string()))
        })
        .map_err(|error| anyhow!("{error}"))
}
