use std::fs;

use serde_json::json;

use super::*;

#[test]
fn archive_and_directory_sources_retain_the_inspected_distribution() {
    for directory in [false, true] {
        let temporary = tempfile::tempdir().unwrap();
        let tree = temporary.path().join("tree");
        fs::create_dir(&tree).unwrap();
        let descriptor = json!({
            "IDAMetadataDescriptorVersion": 1,
            "plugin": {
                "name": "original", "version": "1", "entryPoint": "plugin.py",
                "authors": [{"email": "fixture@example.test"}],
                "urls": {"repository": "https://github.com/example/snapshot"},
            },
        });
        fs::write(tree.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap()).unwrap();
        fs::write(tree.join("plugin.py"), b"original payload").unwrap();
        let path = if directory {
            tree.clone()
        } else {
            let path = temporary.path().join("source.zip");
            fs::write(&path, super::directory::pack(&tree).unwrap()).unwrap();
            path
        };
        let mut distribution = InstallationSource::read(&path, false).unwrap();
        fs::remove_dir_all(&tree).unwrap();
        if !directory {
            fs::write(&path, b"replaced archive").unwrap();
        }
        let metadata = distribution.metadata(None).unwrap();
        assert_eq!(metadata.name, "original");
        assert_eq!(metadata.version, "1");
        let InstallationSource::Archive(mut archive) = distribution else {
            panic!("regular source was not packaged");
        };
        let selected = select_archived_plugin(&mut archive, Some(&metadata.name)).unwrap();
        let extracted = temporary.path().join("extracted");
        super::super::archive::extract(&mut archive, &selected.prefix, &extracted).unwrap();
        assert_eq!(fs::read(extracted.join("plugin.py")).unwrap(), b"original payload");
    }
}
