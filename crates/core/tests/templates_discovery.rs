use markadd_core::templates::discovery::discover_templates;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn discovers_only_md_templates() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("tpl");

    let a = root.join("daily.md");
    let b = root.join("blog").join("post.md");
    let ignored1 = root.join("wiki").join("topic.markdown");
    let ignored2 = root.join("weird.tpl.md");
    let ignored3 = root.join("note.txt");

    write(&a, "# daily");
    write(&b, "# blog");
    write(&ignored1, "# nope");
    write(&ignored2, "# nope");
    write(&ignored3, "# nope");

    let got = discover_templates(&root).expect("discover ok");
    let names: Vec<String> = got.into_iter().map(|t| t.logical_name).collect();

    assert_eq!(names, vec!["blog/post".to_string(), "daily".to_string(),]);
}
