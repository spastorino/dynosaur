use std::path::{Path, PathBuf};

use ui_test::color_eyre::eyre::Result;
use ui_test::dependencies::DependencyBuilder;
use ui_test::spanned::Spanned;
use ui_test::{ignore_output_conflict, run_tests, CommandBuilder, Config};

enum Mode {
    Expand,
    Compile,
    Panic,
}

fn cfg(path: &Path, mode: Mode) -> Config {
    let mut program = CommandBuilder::rustc();

    let exit_status = match mode {
        Mode::Expand => {
            program.args.push("-Zunpretty=expanded".into());
            0
        }

        Mode::Compile => 0,

        Mode::Panic => 101,
    };

    let mut config = Config {
        program,
        ..Config::rustc(path)
    };

    if matches!(mode, Mode::Compile) {
        config.output_conflict_handling = ignore_output_conflict;
    }

    let require_annotations = false; // we're not showing errors in a specific line anyway
    config.comment_defaults.base().exit_status = Spanned::dummy(exit_status).into();
    config.comment_defaults.base().require_annotations = Spanned::dummy(require_annotations).into();
    config.comment_defaults.base().set_custom(
        "dependencies",
        DependencyBuilder {
            crate_manifest_path: PathBuf::from("tests/Cargo.toml"),
            ..DependencyBuilder::default()
        },
    );
    config
}

fn main() -> Result<()> {
    run_tests(cfg(&Path::new("tests/pass"), Mode::Expand))?;
    run_tests(cfg(&Path::new("tests/pass"), Mode::Compile))?;
    run_tests(cfg(&Path::new("tests/fail"), Mode::Panic))
}
