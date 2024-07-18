use std::path::Path;

use ui_test::color_eyre::eyre::Result;
use ui_test::dependencies::DependencyBuilder;
use ui_test::spanned::Spanned;
use ui_test::{run_tests, CommandBuilder, Config};

enum Mode {
    Pass,
    Panic,
}

fn cfg(path: &Path, mode: Mode) -> Config {
    let mut program = CommandBuilder::rustc();
    program.args.push("-Zunpretty=expanded".into());

    let mut config = Config {
        program,
        ..Config::rustc(path)
    };

    let exit_status = match mode {
        Mode::Pass => 0,
        Mode::Panic => 101,
    };
    let require_annotations = false; // we're not showing errors in a specific line anyway
    config.comment_defaults.base().exit_status = Spanned::dummy(exit_status).into();
    config.comment_defaults.base().require_annotations = Spanned::dummy(require_annotations).into();
    config
        .comment_defaults
        .base()
        .set_custom("dependencies", DependencyBuilder::default());
    config
}

fn main() -> Result<()> {
    run_tests(cfg(&Path::new("tests/pass"), Mode::Pass))?;
    run_tests(cfg(&Path::new("tests/fail"), Mode::Panic))
}
