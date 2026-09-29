use std::collections::HashSet;

use video_creater_lib::app_service::operation::{
    find_operation, MutationClass, RemoteSupport, OPERATION_INVENTORY,
};

#[test]
fn operation_names_are_unique_and_resolvable() {
    let mut names = HashSet::new();
    for operation in OPERATION_INVENTORY {
        assert!(names.insert(operation.name), "duplicate {}", operation.name);
        assert_eq!(find_operation(operation.name), Some(operation));
    }
    assert_eq!(OPERATION_INVENTORY.len(), 121);
}

#[test]
fn operation_security_metadata_is_self_consistent() {
    for operation in OPERATION_INVENTORY {
        assert!(operation.max_request_bytes > 0);
        assert!(operation.max_request_bytes <= 32 * 1024 * 1024);
        if operation.support == RemoteSupport::DesktopOnly {
            assert!(
                operation.browser_replacement.is_some(),
                "{}",
                operation.name
            );
        }
        if operation.requires_revision {
            assert!(operation.requires_project, "{}", operation.name);
            assert_ne!(
                operation.mutation,
                MutationClass::Read,
                "{}",
                operation.name
            );
        }
    }
}
