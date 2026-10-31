use crate::error::Result;
use crate::models::{ModDownloadPlanItem, ModDownloadSelection};
use crate::services::{curseforge, modrinth, mods, profiles};
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_DEPTH: usize = 6;

struct DependencyRef {
    project_id: String,
    version_id: Option<String>,
}

struct WorkItem {
    selection: ModDownloadSelection,
    required_by: Option<String>,
    is_dependency: bool,
    depth: usize,
}

fn project_key(source: &str, mod_id: &str) -> String {
    format!("{source}:{mod_id}")
}

fn merge_required_by(existing: &mut Option<String>, parent: &str) {
    match existing {
        Some(cur) if cur.split(", ").any(|part| part == parent) => {}
        Some(cur) => *existing = Some(format!("{cur}, {parent}")),
        None => *existing = Some(parent.to_string()),
    }
}

fn to_plan_item(
    selection: &ModDownloadSelection,
    required_by: Option<String>,
    is_dependency: bool,
) -> ModDownloadPlanItem {
    ModDownloadPlanItem {
        mod_id: selection.mod_id.clone(),
        version_id: selection.version_id.clone(),
        source: selection.source.clone(),
        title: selection.title.clone(),
        version_name: selection.version_name.clone(),
        filename: selection.filename.clone(),
        download_url: selection.download_url.clone(),
        size_bytes: selection.size_bytes,
        required_by,
        is_dependency,
        already_installed: false,
    }
}

pub async fn plan_downloads(
    profile_id: &str,
    selections: Vec<ModDownloadSelection>,
) -> Result<Vec<ModDownloadPlanItem>> {
    let profile = profiles::get(profile_id)?;
    let installed = mods::installed_filenames(profile_id);
    let mut plan: Vec<ModDownloadPlanItem> = Vec::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut seen_projects: HashSet<String> = HashSet::new();
    let mut plan_index: HashMap<String, usize> = HashMap::new();

    let mut queue: VecDeque<WorkItem> = selections
        .into_iter()
        .map(|selection| WorkItem {
            selection,
            required_by: None,
            is_dependency: false,
            depth: 0,
        })
        .collect();

    while let Some(work) = queue.pop_front() {
        let selection = work.selection;
        if work.depth > MAX_DEPTH {
            continue;
        }

        let project = project_key(&selection.source, &selection.mod_id);
        let mut fetch_deps = true;

        if seen_projects.contains(&project) {
            if let Some(&idx) = plan_index.get(&project) {
                let user_replaces_dep = !work.is_dependency && plan[idx].is_dependency;
                if user_replaces_dep {
                    log::info!(
                        "Using selected {} {} instead of dependency {}",
                        selection.title,
                        selection.version_name,
                        plan[idx].version_name
                    );
                    seen_files.remove(&plan[idx].filename);
                    let required_by = plan[idx].required_by.clone();
                    let already_installed = mods::mark_installed(&installed, &selection.filename)
                        || seen_files.contains(&selection.filename);
                    if already_installed {
                        plan.remove(idx);
                        rebuild_plan_index(&plan, &mut plan_index);
                    } else {
                        seen_files.insert(selection.filename.clone());
                        plan[idx] = to_plan_item(&selection, required_by, false);
                    }
                } else {
                    if let Some(parent) = &work.required_by {
                        merge_required_by(&mut plan[idx].required_by, parent);
                    }
                    log::info!(
                        "Skipping extra {} {} (already have {})",
                        selection.title,
                        selection.version_name,
                        plan[idx].version_name
                    );
                    fetch_deps = false;
                }
            } else {
                fetch_deps = false;
            }
        } else {
            seen_projects.insert(project.clone());
            let already_installed = mods::mark_installed(&installed, &selection.filename)
                || seen_files.contains(&selection.filename);
            if !already_installed {
                seen_files.insert(selection.filename.clone());
                plan_index.insert(project, plan.len());
                plan.push(to_plan_item(
                    &selection,
                    work.required_by.clone(),
                    work.is_dependency,
                ));
            }
        }

        if !fetch_deps {
            continue;
        }

        let deps: Vec<DependencyRef> = match selection.source.as_str() {
            "modrinth" => modrinth::fetch_version_dependencies(&selection.version_id)
                .await?
                .into_iter()
                .map(|d| DependencyRef {
                    project_id: d.project_id,
                    version_id: d.version_id,
                })
                .collect(),
            "curseforge" => {
                curseforge::fetch_file_dependencies(&selection.mod_id, &selection.version_id)
                    .await?
                    .into_iter()
                    .map(|d| DependencyRef {
                        project_id: d.project_id,
                        version_id: None,
                    })
                    .collect()
            }
            _ => vec![],
        };

        for dep in deps {
            let dep_selection = match selection.source.as_str() {
                "modrinth" => {
                    modrinth::resolve_dependency(
                        profile_id,
                        &dep.project_id,
                        dep.version_id.as_deref(),
                        &profile.loader,
                        &profile.mc_version,
                    )
                    .await?
                }
                "curseforge" => {
                    curseforge::resolve_dependency(
                        profile_id,
                        &dep.project_id,
                        &profile.loader,
                        &profile.mc_version,
                    )
                    .await?
                }
                _ => None,
            };

            if let Some(dep_sel) = dep_selection {
                queue.push_back(WorkItem {
                    selection: dep_sel,
                    required_by: Some(selection.title.clone()),
                    is_dependency: true,
                    depth: work.depth + 1,
                });
            }
        }
    }

    Ok(plan)
}

fn rebuild_plan_index(plan: &[ModDownloadPlanItem], plan_index: &mut HashMap<String, usize>) {
    plan_index.clear();
    for (idx, item) in plan.iter().enumerate() {
        plan_index.insert(project_key(&item.source, &item.mod_id), idx);
    }
}

pub async fn install_plan(profile_id: &str, items: &[ModDownloadPlanItem]) -> Result<usize> {
    let mut installed = 0usize;
    for item in items {
        if item.already_installed {
            continue;
        }
        mods::install_from_url(profile_id, &item.filename, &item.download_url).await?;
        installed += 1;
    }
    Ok(installed)
}

pub async fn install_modrinth_version_dependencies(
    profile_id: &str,
    version_id: &str,
    parent_title: &str,
) -> Result<usize> {
    let version = modrinth::fetch_version(version_id).await?;
    let selection = ModDownloadSelection {
        mod_id: version.project_id,
        version_id: version.id,
        source: "modrinth".into(),
        title: parent_title.into(),
        version_name: version.name,
        filename: version.filename,
        download_url: version.download_url,
        size_bytes: version.size_bytes,
    };

    let plan = plan_downloads(profile_id, vec![selection]).await?;
    let to_install: Vec<ModDownloadPlanItem> = plan
        .into_iter()
        .filter(|item| item.is_dependency && !item.already_installed)
        .collect();

    install_plan(profile_id, &to_install).await
}
