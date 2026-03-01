use std::env;

use snapbox::cmd::Command;

macro_rules! mktest {
    ($name:ident, $case:expr) => {
        #[test]
        fn $name() -> gix_testtools::Result {
            let path = gix_testtools::scripted_fixture_read_only(format!("{}.sh", $case))?;

            Command::new(snapbox::cmd::cargo_bin!("git-tree"))
                .current_dir(path)
                .assert()
                .success()
                .stdout_eq(snapbox::file![_: TermSvg]);

            Ok(())
        }
    };
}

mktest!(no_changes, "no_changes");
mktest!(some_changes, "some_changes");
mktest!(some_staged_changes, "some_staged_changes");
