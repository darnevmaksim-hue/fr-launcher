use crate::models::{McVersionGroup, McVersionSupport};

struct VersionGroupDef {
    label: &'static str,
    mod_version: &'static str,
    versions: &'static [&'static str],
    loaders: &'static [&'static str],
    java: u8,
}

const GROUPS: &[VersionGroupDef] = &[
    VersionGroupDef {
        label: "1.20.1",
        mod_version: "1.20.1",
        versions: &["1.20.1"],
        loaders: &["fabric", "quilt", "forge"],
        java: 17,
    },
    VersionGroupDef {
        label: "1.20.2",
        mod_version: "1.20.2",
        versions: &["1.20.2"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 17,
    },
    VersionGroupDef {
        label: "1.20.3 – 1.20.4",
        mod_version: "1.20.4",
        versions: &["1.20.3", "1.20.4"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 17,
    },
    VersionGroupDef {
        label: "1.20.5 – 1.20.6",
        mod_version: "1.20.6",
        versions: &["1.20.5", "1.20.6"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21 – 1.21.1",
        mod_version: "1.21.1",
        versions: &["1.21", "1.21.1"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21.2 – 1.21.3",
        mod_version: "1.21.3",
        versions: &["1.21.2", "1.21.3"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21.4",
        mod_version: "1.21.4",
        versions: &["1.21.4"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21.5",
        mod_version: "1.21.5",
        versions: &["1.21.5"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21.6 – 1.21.9",
        mod_version: "1.21.6",
        versions: &["1.21.6", "1.21.7", "1.21.8", "1.21.9"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "1.21.10 – 1.21.11",
        mod_version: "1.21.10",
        versions: &["1.21.10", "1.21.11"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 21,
    },
    VersionGroupDef {
        label: "26.1",
        mod_version: "26.1",
        versions: &["26.1"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 25,
    },
    VersionGroupDef {
        label: "26.2",
        mod_version: "26.2",
        versions: &["26.2"],
        loaders: &["fabric", "quilt", "forge", "neoforge"],
        java: 25,
    },
];

pub fn supported_version_groups() -> Vec<McVersionGroup> {
    GROUPS
        .iter()
        .map(|def| McVersionGroup {
            id: def.mod_version.into(),
            label: def.label.into(),
            mod_version: def.mod_version.into(),
            versions: def.versions.iter().map(|v| (*v).into()).collect(),
            loaders: def.loaders.iter().map(|l| (*l).into()).collect(),
            java: def.java,
        })
        .collect()
}

pub fn supported_versions() -> Vec<McVersionSupport> {
    GROUPS
        .iter()
        .flat_map(|def| {
            def.versions.iter().map(|ver| McVersionSupport {
                version: (*ver).into(),
                loaders: def.loaders.iter().map(|l| (*l).into()).collect(),
                java: def.java,
                mod_version: def.mod_version.into(),
                group_label: def.label.into(),
            })
        })
        .collect()
}

pub fn mod_mc_version(mc: &str) -> String {
    GROUPS
        .iter()
        .find(|g| g.versions.iter().any(|v| *v == mc))
        .map(|g| g.mod_version.to_string())
        .unwrap_or_else(|| mc.to_string())
}

pub fn compatible_mc_versions(mc: &str) -> Vec<String> {
    GROUPS
        .iter()
        .find(|g| g.versions.iter().any(|v| *v == mc) || g.mod_version == mc)
        .map(|g| {
            let mut versions: Vec<String> = g.versions.iter().map(|v| (*v).to_string()).collect();
            if !versions.iter().any(|v| v == g.mod_version) {
                versions.push(g.mod_version.to_string());
            }
            versions
        })
        .unwrap_or_else(|| vec![mc.to_string()])
}

pub fn is_supported(mc: &str, loader: &str) -> bool {
    supported_versions()
        .iter()
        .any(|v| v.version == mc && v.loaders.iter().any(|l| l == loader))
}

pub fn required_java(mc: &str) -> u8 {
    supported_versions()
        .into_iter()
        .find(|v| v.version == mc)
        .map(|v| v.java)
        .unwrap_or(21)
}
