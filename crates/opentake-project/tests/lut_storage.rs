#[allow(dead_code)]
mod common;

use common::TempDir;
use opentake_project::{Project, ProjectRoot};

#[test]
fn managed_lut_is_bounded_nofollow_and_carried_by_complete_save_as() {
    let temp = TempDir::new("lut-storage");
    let source = temp.child("Source.opentake");
    Project::new(&source).save().expect("create source bundle");
    let source_root = ProjectRoot::open(&source).expect("retain source root");
    let name = format!("{}.cube", "0123456789abcdef".repeat(4));
    let bytes = b"LUT_3D_SIZE 17\n# acceptance bytes\n";
    source_root
        .write_lut_atomic(&name, bytes)
        .expect("publish managed LUT");
    assert_eq!(
        source_root.read_lut(&name, 4096).unwrap().as_deref(),
        Some(bytes.as_slice())
    );
    assert!(
        source_root.read_lut(&name, 4).is_err(),
        "read cap is enforced"
    );

    let destination = temp.child("Destination.opentake");
    let destination_root = Project::new(&destination)
        .publish_complete_to(&destination, Some(&source_root))
        .expect("complete Save As");
    assert_eq!(
        destination_root.read_lut(&name, 4096).unwrap().as_deref(),
        Some(bytes.as_slice()),
        "nested media/luts asset must travel with Save As"
    );
}
