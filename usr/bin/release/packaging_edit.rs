//! Version string transforms for packaging manifests.

pub(super) fn set_nix_version(content: &str, version: &str) -> String {
    replace_line_value(content, "version = ", &format!("\"{version}\";"))
}

pub(super) fn set_homebrew_url(content: &str, version: &str) -> String {
    let prefix = "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v";
    replace_prefixed_line(content, prefix, &format!("{prefix}{version}.tar.gz\""))
}

pub(super) fn set_flatpak_tag(content: &str, version: &str) -> String {
    replace_line_value(content, "tag: v", version)
}

pub(super) fn set_pkgver(content: &str, version: &str) -> String {
    replace_line_value(content, "pkgver=", version)
}

pub(super) fn set_distversion(content: &str, version: &str) -> String {
    replace_line_value(content, "DISTVERSION=\t", version)
}

pub(super) fn set_cargo_package_version(content: &str, version: &str) -> String {
    replace_line_value(content, "version = ", &format!("\"{version}\""))
}

pub(super) fn set_scout_lib_dep_version(content: &str, version: &str) -> String {
    let marker = "scout_lib = { path = \"../scout_lib\"";
    content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with(marker) {
                return line.to_string();
            }
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            let rest = &trimmed[marker.len()..];
            let without_version = if let Some(after_comma) = rest.strip_prefix(", version = \"") {
                match after_comma.find('"') {
                    Some(end) => &after_comma[end + 1..],
                    None => rest,
                }
            } else {
                rest
            };
            format!("{indent}{marker}, version = \"{version}\"{without_version}")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

pub(super) fn set_gentoo_readme_ebuild(content: &str, version: &str) -> String {
    let pattern = "scout-cli-";
    content
        .lines()
        .map(|line| {
            if let Some(index) = line.find(pattern) {
                let suffix_start = index + pattern.len();
                if let Some(end) = line[suffix_start..].find(".ebuild") {
                    let mut next = line.to_string();
                    next.replace_range(suffix_start..suffix_start + end, version);
                    return next;
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn replace_line_value(content: &str, prefix: &str, value: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with(prefix) {
                let indent = line.len() - line.trim_start().len();
                format!("{}{prefix}{value}", &line[..indent])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn replace_prefixed_line(content: &str, prefix: &str, replacement: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.starts_with(prefix) || line.contains(prefix) {
                replacement.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_nix_version_replaces_existing_value() {
        let input = "  version = \"0.1.0\";\n";
        assert_eq!(set_nix_version(input, "0.2.0"), "  version = \"0.2.0\";\n");
    }

    #[test]
    fn set_homebrew_url_replaces_tagged_archive_url() {
        let input =
            "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.1.0.tar.gz\"\n";
        let expected =
            "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.2.0.tar.gz\"";
        assert_eq!(set_homebrew_url(input, "0.2.0"), format!("{expected}\n"));
    }

    #[test]
    fn set_gentoo_readme_ebuild_updates_filename_reference() {
        let input = "Template: scout-cli-0.1.0.ebuild\n";
        assert_eq!(
            set_gentoo_readme_ebuild(input, "0.2.0"),
            "Template: scout-cli-0.2.0.ebuild\n"
        );
    }

    #[test]
    fn set_scout_lib_dep_version_inserts_version_for_publish() {
        let input = "scout_lib = { path = \"../scout_lib\", default-features = false }\n";
        let expected =
            "scout_lib = { path = \"../scout_lib\", version = \"0.6.0\", default-features = false }\n";
        assert_eq!(set_scout_lib_dep_version(input, "0.6.0"), expected);
    }

    #[test]
    fn set_scout_lib_dep_version_replaces_existing_version() {
        let input =
            "scout_lib = { path = \"../scout_lib\", version = \"0.5.0\", default-features = false }\n";
        let expected =
            "scout_lib = { path = \"../scout_lib\", version = \"0.6.0\", default-features = false }\n";
        assert_eq!(set_scout_lib_dep_version(input, "0.6.0"), expected);
    }
}
