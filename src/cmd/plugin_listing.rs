//! Repository ordering and install references shared by plugin listings.

pub(super) fn repository_order(repository: Option<&str>) -> (bool, bool, &str) {
    (repository.is_none(), repository == Some("community"), repository.unwrap_or(""))
}

pub(super) fn install_reference(name: &str, repository: Option<&str>, default: &str) -> String {
    match repository.filter(|repository| *repository != default) {
        Some(repository) => format!("{repository}/{name}"),
        None => name.into(),
    }
}
