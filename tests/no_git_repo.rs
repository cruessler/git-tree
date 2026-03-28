use gix_testtools::tempfile::tempdir;
use snapbox::cmd::Command;

#[test]
fn no_git_repo() -> gix_testtools::Result {
    let empty_dir = tempdir()?;

    Command::new(snapbox::cmd::cargo_bin!("git-tree"))
        .current_dir(empty_dir.path())
        .assert()
        .success()
        .stdout_eq(snapbox::file![_: TermSvg]);

    Ok(())
}
