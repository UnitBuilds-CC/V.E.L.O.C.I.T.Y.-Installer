//! Component tree view logic with disk space display.
//!
//! Provides helpers for rendering the installable component hierarchy
//! in the wizard UI, including tree traversal, size calculation, and
//! dependency resolution.

use velocity_config::Component;

/// A flattened tree node ready for display in a list or tree control.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Component identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Description text
    pub description: String,
    /// Depth level (0 = root)
    pub depth: u32,
    /// Disk space required (bytes)
    pub size: u64,
    /// Whether this node is selected by default
    pub selected: bool,
    /// Whether this node is mandatory (cannot be deselected)
    pub mandatory: bool,
    /// Whether this node has children
    pub has_children: bool,
    /// Component IDs this node depends on
    pub depends_on: Vec<String>,
}

/// Flatten a component tree into a list of `TreeNode`s for display.
///
/// The resulting vector preserves tree order (parent before children)
/// and includes depth information for indentation.
pub fn flatten_component_tree(components: &[Component]) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    for comp in components {
        flatten_recursive(comp, 0, &mut nodes);
    }
    nodes
}

fn flatten_recursive(comp: &Component, depth: u32, out: &mut Vec<TreeNode>) {
    let has_children = !comp.children.is_empty();
    out.push(TreeNode {
        id: comp.id.clone(),
        name: comp.name.clone(),
        description: comp.description.clone().unwrap_or_default(),
        depth,
        size: comp.size,
        selected: comp.selected_by_default || comp.mandatory,
        mandatory: comp.mandatory,
        has_children,
        depends_on: comp.depends_on.clone(),
    });
    for child in &comp.children {
        flatten_recursive(child, depth + 1, out);
    }
}

/// Calculate total disk space required for a set of selected component IDs.
///
/// Includes sizes of selected components and their children.
pub fn calculate_total_size(components: &[Component], selected_ids: &[String]) -> u64 {
    let mut total: u64 = 0;
    for comp in components {
        if component_is_selected(comp, selected_ids) {
            total += component_total_size(comp);
        }
    }
    total
}

/// Check whether a component (or its parent) is in the selected list.
fn component_is_selected(comp: &Component, selected_ids: &[String]) -> bool {
    selected_ids.contains(&comp.id)
}

/// Calculate total size of a component including all its children.
fn component_total_size(comp: &Component) -> u64 {
    let mut size = comp.size;
    for child in &comp.children {
        size += component_total_size(child);
    }
    size
}

/// Format a byte count as a human-readable string (e.g., "12.3 MB").
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Resolve dependencies: given a set of selected component IDs, ensure that
/// all dependency targets are also included.
///
/// Returns a new set of IDs with dependencies added.
pub fn resolve_dependencies(
    components: &[Component],
    selected_ids: &[String],
) -> Vec<String> {
    let flat = flatten_component_tree(components);
    let mut result: Vec<String> = selected_ids.to_vec();

    // Iteratively add dependencies until stable
    let mut changed = true;
    while changed {
        changed = false;
        for node in &flat {
            if result.contains(&node.id) {
                for dep in &node.depends_on {
                    if !result.contains(dep) {
                        result.push(dep.clone());
                        changed = true;
                    }
                }
            }
        }
    }

    result
}

/// Build a display string for a tree node with indentation and size.
///
/// Example: "    Documentation (2.5 MB)"
pub fn format_node_display(node: &TreeNode) -> String {
    let indent = "    ".repeat(node.depth as usize);
    if node.size > 0 {
        format!("{}{} ({})", indent, node.name, format_size(node.size))
    } else {
        format!("{}{}", indent, node.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_component(id: &str, name: &str, size: u64, mandatory: bool) -> Component {
        Component {
            id: id.to_string(),
            name: name.to_string(),
            description: Some(format!("{} description", name)),
            selected_by_default: true,
            mandatory,
            size,
            group: None,
            files: vec![],
            install_subdir: None,
            registry: vec![],
            shortcuts: vec![],
            children: vec![],
            depends_on: vec![],
        }
    }

    #[test]
    fn test_flatten_empty() {
        let result = flatten_component_tree(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_flatten_single() {
        let comps = vec![make_component("core", "Core Files", 10_000_000, false)];
        let nodes = flatten_component_tree(&comps);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "Core Files");
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes[0].size, 10_000_000);
    }

    #[test]
    fn test_flatten_with_children() {
        let mut parent = make_component("app", "Application", 5_000_000, false);
        parent.children = vec![
            make_component("docs", "Documentation", 1_000_000, false),
            make_component("sdk", "SDK", 3_000_000, false),
        ];
        let nodes = flatten_component_tree(&[parent]);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 1);
    }

    #[test]
    fn test_calculate_total_size() {
        let comps = vec![
            make_component("core", "Core", 10_000_000, false),
            make_component("docs", "Docs", 2_000_000, false),
        ];
        let total = calculate_total_size(&comps, &["core".into(), "docs".into()]);
        assert_eq!(total, 12_000_000);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
        assert_eq!(format_size(2_000_000_000), "1.9 GB");
    }

    #[test]
    fn test_resolve_dependencies() {
        let mut sdk = make_component("sdk", "SDK", 3_000_000, false);
        sdk.depends_on = vec!["core".to_string()];
        let comps = vec![
            make_component("core", "Core", 10_000_000, false),
            sdk,
        ];
        let resolved = resolve_dependencies(&comps, &["sdk".into()]);
        assert!(resolved.contains(&"core".to_string()));
        assert!(resolved.contains(&"sdk".to_string()));
    }

    #[test]
    fn test_format_node_display() {
        let node = TreeNode {
            id: "docs".into(),
            name: "Documentation".into(),
            description: String::new(),
            depth: 1,
            size: 2_500_000,
            selected: true,
            mandatory: false,
            has_children: false,
            depends_on: vec![],
        };
        let display = format_node_display(&node);
        assert!(display.contains("Documentation"));
        assert!(display.contains("2.4 MB"));
        assert!(display.starts_with("    ")); // indented
    }
}
