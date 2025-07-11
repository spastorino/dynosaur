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
    let mut config = Config {
        program: CommandBuilder::rustc(),
        output_conflict_handling: if std::env::var_os("BLESS").is_some() {
            ui_test::bless_output_files
        } else {
            ui_test::error_on_output_conflict
        },
        ..Config::rustc(path)
    };

    let exit_status = match mode {
        Mode::Expand => {
            config.program.args.push("-Zunpretty=expanded".into());
            0
        }

        Mode::Compile => {
            config.output_conflict_handling = ignore_output_conflict;
            0
        }

        Mode::Panic => {
            config.program.args.push("-Zunpretty=expanded".into());
            1
        }
    };

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
