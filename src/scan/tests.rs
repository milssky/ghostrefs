use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, process};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("ghostrefs-scan-{}-{unique}", process::id()));
        fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the scan regression scenario together"
)]
fn package_copies_do_not_become_workspace_consumers() {
    let workspace = Workspace::new();
    let source = workspace.write("app/services.py", "def helper(): pass\n");
    let tests = workspace.write(
        "tests/test_services.py",
        "from app.services import helper\n",
    );
    let consumer = workspace.write("scripts/consumer.py", "from app.services import helper\n");
    let _ = workspace.write(
        "target/package/old-release/tests/test_services.py",
        "from app.services import helper\n",
    );
    let _ = workspace.write(
        "target/package/old-release/app/services.py",
        "def helper(): pass\n",
    );

    let files = scan_workspace(
        &workspace.0,
        &workspace.0.join("app"),
        &workspace.0.join("tests"),
    )
    .unwrap();
    let mut expected = vec![source, tests, consumer];
    expected.sort();
    assert_eq!(files, expected);
}
