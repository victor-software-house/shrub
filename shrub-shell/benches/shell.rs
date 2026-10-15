//! Benchmarks for the shrub-shell crate.

#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

#[cfg(unix)]
mod unix {
    use shrub_builtins::ShellBuilderExt;
    use shrub_parser::SourceSpan;
    use criterion::Criterion;
    use std::hint::black_box;

    async fn instantiate_shell() -> shrub_core::Shell {
        shrub_core::Shell::builder()
            .default_builtins(shrub_builtins::BuiltinSet::BashMode)
            .build()
            .await
            .unwrap()
    }

    async fn instantiate_shell_with_init_scripts() -> shrub_core::Shell {
        shrub_core::Shell::builder()
            .interactive(true)
            .read_commands_from_stdin(true)
            .default_builtins(shrub_builtins::BuiltinSet::BashMode)
            .build()
            .await
            .unwrap()
    }

    async fn run_one_command(shell: &mut shrub_core::Shell, command: &str) {
        let _ = shell
            .run_string(
                command.to_owned(),
                &shrub_core::SourceInfo::default(),
                &shell.default_exec_params(),
            )
            .await
            .unwrap();
    }

    async fn expand_string(shell: &mut shrub_core::Shell, s: &str) {
        let params = shell.default_exec_params();
        let _ = shell.basic_expand_string(&params, s).await.unwrap();
    }

    fn eval_arithmetic_expr(shell: &mut shrub_core::Shell, expr: &str) {
        let parsed_expr = shrub_parser::arithmetic::parse(expr).unwrap();
        let _ = shell.eval_arithmetic(&parsed_expr).unwrap();
    }

    /// This function defines core shell benchmarks.
    pub(crate) fn criterion_benchmark(c: &mut Criterion) {
        // Construct a runtime for us to run async code on.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Benchmark shell instantiation.
        c.bench_function("instantiate_shell", |b| {
            b.to_async(&rt).iter(|| black_box(instantiate_shell()));
        });
        c.bench_function("instantiate_shell_with_init_scripts", |b| {
            b.to_async(&rt)
                .iter(|| black_box(instantiate_shell_with_init_scripts()));
        });

        // Benchmark: cloning a shell object.
        let shell = rt.block_on(instantiate_shell());
        c.bench_function("clone_shell_object", |b| {
            b.iter(|| black_box(shell.clone()));
        });

        // Benchmark: parsing and evaluating an arithmetic expression..
        let shell = rt.block_on(instantiate_shell());
        c.bench_function("eval_arithmetic", |b| {
            b.iter_batched_ref(
                || shell.clone(),
                |s| eval_arithmetic_expr(s, "3 + 10 * 2"),
                criterion::BatchSize::SmallInput,
            );
        });

        // Benchmark: running the echo built-in command.
        let shell = rt.block_on(instantiate_shell());
        c.bench_function("run_echo_builtin_command", |b| {
            b.iter_batched_ref(
                || shell.clone(),
                |s| rt.block_on(run_one_command(s, "echo 'Hello, world!' >/dev/null")),
                criterion::BatchSize::SmallInput,
            );
        });

        // Benchmark: running an external command.
        // let shell = rt.block_on(instantiate_shell());
        // c.bench_function("run_one_external_command", |b| {
        //     b.iter_batched_ref(
        //         || shell.clone(),
        //         |s| {
        //             rt.block_on(run_one_command(
        //                 s,
        //                 "/usr/bin/echo 'Hello, world!' >/dev/null",
        //             ));
        //         },
        //         criterion::BatchSize::SmallInput,
        //     );
        // });

        // Benchmark: word expansion.
        let shell = rt.block_on(instantiate_shell());
        c.bench_function("expand_one_string", |b| {
            b.iter_batched_ref(
                || shell.clone(),
                |s| rt.block_on(expand_string(s, "My version is ${BASH_VERSINFO[@]}")),
                criterion::BatchSize::SmallInput,
            );
        });

        // Benchmark: function invocation.
        let mut shell = rt.block_on(instantiate_shell());
        shell.define_func(
            String::from("testfunc"),
            shrub_parser::ast::FunctionDefinition {
                fname: String::from("testfunc").into(),
                body: shrub_parser::ast::FunctionBody(
                    shrub_parser::ast::CompoundCommand::BraceGroup(
                        shrub_parser::ast::BraceGroupCommand {
                            list: shrub_parser::ast::CompoundList(vec![]),
                            loc: SourceSpan::default(),
                        },
                    ),
                    None,
                ),
            },
            &shrub_core::SourceInfo::default(),
        );
        c.bench_function("function_call", |b| {
            b.iter_batched_ref(
                || shell.clone(),
                |s| {
                    rt.block_on(run_one_command(s, "testfunc"));
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // Benchmark: for loop.
        let shell = rt.block_on(instantiate_shell());
        c.bench_function("for_loop", |b| {
            b.iter_batched_ref(
                || shell.clone(),
                |s| {
                    rt.block_on(run_one_command(s, "for ((i = 0; i < 10; i++)); do :; done"));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
}

#[cfg(unix)]
criterion::criterion_group! {
    name = benches;
    config = criterion::Criterion::default()
                .measurement_time(std::time::Duration::from_secs(10));
    targets = unix::criterion_benchmark
}

#[cfg(unix)]
criterion::criterion_main!(benches);

#[cfg(not(unix))]
fn main() {}
