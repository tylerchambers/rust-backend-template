use application::{Actor, Projects};
use domain::{Error, OrganizationId};

/// Identical observable application/port contract for memory and PostgreSQL.
pub async fn project_contract(
    service: &Projects,
    member: &Actor,
    stranger: &Actor,
    org: OrganizationId,
) {
    let created = service
        .create(member, org, "Contract project".into())
        .await
        .unwrap();
    assert_eq!(created.organization_id, org);
    assert_eq!(created.version, 1);
    let renamed = service
        .rename(member, org, created.id, 1, "Renamed".into())
        .await
        .unwrap();
    assert_eq!(renamed.version, 2);
    assert_eq!(renamed.name, "Renamed");
    let detail = service.get(member, org, created.id).await.unwrap();
    assert_eq!(detail.id, created.id);
    assert_eq!(detail.organization_id, org);
    assert_eq!(detail.name, renamed.name);
    assert_eq!(detail.version, renamed.version);
    assert_eq!(detail.archived, renamed.archived);
    assert!(matches!(
        service
            .rename(member, org, created.id, 1, "Stale".into())
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service.create(stranger, org, "Forbidden".into()).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.create(member, org, String::new()).await,
        Err(Error::Invalid)
    ));
    let rows = service.list(member, org, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Renamed");
    assert_eq!(rows[0].version, 2);
    assert!(
        service
            .list(member, org, Some(created.id))
            .await
            .unwrap()
            .is_empty()
    );
}
