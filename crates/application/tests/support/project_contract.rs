use application::{Actor, Projects};
use domain::{Error, OrganizationId};

/// Identical observable application/port contract for memory and PostgreSQL.
pub async fn project_contract(
    service: &Projects,
    owner: &Actor,
    stranger: &Actor,
    org: OrganizationId,
) {
    let created = service
        .create(owner, org, "Contract project".into())
        .await
        .unwrap();
    assert_eq!(created.organization_id, org);
    assert_eq!(created.version, 1);
    let renamed = service
        .rename(owner, org, created.id, 1, "Renamed".into())
        .await
        .unwrap();
    assert_eq!(renamed.version, 2);
    assert_eq!(renamed.name, "Renamed");
    let detail = service.get(owner, org, created.id).await.unwrap();
    assert_eq!(detail.id, created.id);
    assert_eq!(detail.organization_id, org);
    assert_eq!(detail.name, renamed.name);
    assert_eq!(detail.version, renamed.version);
    assert_eq!(detail.archived, renamed.archived);
    assert!(matches!(
        service
            .rename(owner, org, created.id, 1, "Stale".into())
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service.create(stranger, org, "Forbidden".into()).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.create(owner, org, String::new()).await,
        Err(Error::Invalid)
    ));
    let rows = service.list(owner, org, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Renamed");
    assert_eq!(rows[0].version, 2);
    assert!(
        service
            .list(owner, org, Some(created.id))
            .await
            .unwrap()
            .is_empty()
    );
}
