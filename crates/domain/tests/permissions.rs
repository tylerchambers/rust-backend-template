use domain::{Permission, Role};
#[test]
fn role_matrix_is_deny_by_default() {
    let roles = [Role::Owner, Role::Admin, Role::Member, Role::Viewer];
    for (role, expected) in roles.into_iter().zip([
        [true, true, true, true],
        [true, true, true, false],
        [true, true, false, false],
        [true, false, false, false],
    ]) {
        for (permission, allow) in [
            Permission::ReadProject,
            Permission::WriteProject,
            Permission::ManageMembers,
            Permission::ManageOwners,
        ]
        .into_iter()
        .zip(expected)
        {
            assert_eq!(role.allows(permission), allow);
        }
    }
}
