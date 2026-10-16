use crate::log::SignalInfo;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Clone, Debug, Default, PartialEq)]
struct Node {
    children: BTreeMap<String, Node>,
    signal: Option<SignalInfo>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldTree(BTreeMap<String, Node>);
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub count: usize,
    pub field: Option<String>,
    pub open: bool,
    pub last: bool,
    pub guides: Vec<usize>,
}
impl FieldTree {
    pub fn new(signals: &[SignalInfo]) -> Self {
        let mut tree = Self::default();
        for signal in signals {
            let mut path = signal.id.splitn(3, '/');
            let (Some(topic), Some(instance), Some(field)) =
                (path.next(), path.next(), path.next())
            else {
                continue;
            };
            let mut node = tree.0.entry(format!("{topic}.{instance}")).or_default();
            for part in field.split('.') {
                if let Some(index) = part.find('[') {
                    if index > 0 {
                        node = node.children.entry(part[..index].to_owned()).or_default();
                    }
                    for index in part[index..].split_inclusive(']') {
                        node = node.children.entry(index.to_owned()).or_default();
                    }
                } else {
                    node = node.children.entry(part.to_owned()).or_default();
                }
            }
            node.signal = Some(signal.clone());
        }
        let mut instances = BTreeMap::<String, usize>::new();
        for label in tree.0.keys() {
            let (topic, _) = label.rsplit_once('.').expect("topic-instance root");
            *instances.entry(topic.to_owned()).or_default() += 1;
        }
        Self(
            tree.0
                .into_iter()
                .map(|(label, node)| {
                    let (topic, _) = label.rsplit_once('.').expect("topic-instance root");
                    let label = if instances[topic] == 1 {
                        topic.to_owned()
                    } else {
                        label
                    };
                    (label, node)
                })
                .collect(),
        )
    }
    pub fn rows(&self, query: &str, expanded: &BTreeSet<String>) -> Vec<TreeRow> {
        let query = query.trim().to_ascii_lowercase();
        let mut rows = Vec::new();
        let filtered = (!query.is_empty()).then(|| {
            self.0
                .iter()
                .filter_map(|(label, node)| {
                    filter(node, label, &query).map(|node| (label.clone(), node))
                })
                .collect()
        });
        for (label, node) in ordered(filtered.as_ref().unwrap_or(&self.0)) {
            append(node, label, label, 0, expanded, &mut rows);
        }
        connect(&mut rows);
        rows
    }
}
fn filter(node: &Node, label: &str, query: &str) -> Option<Node> {
    if label.to_ascii_lowercase().contains(query)
        || node
            .signal
            .as_ref()
            .is_some_and(|signal| signal.id.to_ascii_lowercase().contains(query))
    {
        return Some(node.clone());
    }
    let children: BTreeMap<_, _> = node
        .children
        .iter()
        .filter_map(|(label, child)| filter(child, label, query).map(|node| (label.clone(), node)))
        .collect();
    (!children.is_empty()).then_some(Node {
        children,
        signal: None,
    })
}
fn ordered(nodes: &BTreeMap<String, Node>) -> Vec<(&String, &Node)> {
    let mut nodes: Vec<_> = nodes.iter().collect();
    nodes.sort_by(|(a, _), (b, _)| natural_cmp(a, b));
    nodes
}
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    while !a.is_empty() && !b.is_empty() {
        if a[0].is_ascii_digit() && b[0].is_ascii_digit() {
            let (an, bn) = (
                a.iter().take_while(|c| c.is_ascii_digit()).count(),
                b.iter().take_while(|c| c.is_ascii_digit()).count(),
            );
            let (av, bv) = (&a[..an], &b[..bn]);
            let (az, bz) = (
                av.iter().take_while(|c| **c == b'0').count(),
                bv.iter().take_while(|c| **c == b'0').count(),
            );
            let order = (an - az)
                .cmp(&(bn - bz))
                .then_with(|| av[az..].cmp(&bv[bz..]))
                .then_with(|| an.cmp(&bn));
            if order != Ordering::Equal {
                return order;
            }
            a = &a[an..];
            b = &b[bn..];
        } else {
            let order = a[0].cmp(&b[0]);
            if order != Ordering::Equal {
                return order;
            }
            a = &a[1..];
            b = &b[1..];
        }
    }
    a.len().cmp(&b.len())
}
fn connect(rows: &mut [TreeRow]) {
    let mut next = Vec::new();
    for row in rows.iter_mut().rev() {
        while next.last().is_some_and(|depth| *depth > row.depth) {
            next.pop();
        }
        row.last = next.last() != Some(&row.depth);
        next.push(row.depth);
    }
    let mut ancestors: Vec<bool> = Vec::new();
    for row in rows {
        ancestors.truncate(row.depth);
        row.guides = ancestors
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(depth, last)| (!*last).then_some(depth - 1))
            .collect();
        ancestors.push(row.last);
    }
}
fn count(node: &Node) -> usize {
    usize::from(node.signal.is_some()) + node.children.values().map(count).sum::<usize>()
}
fn append(
    node: &Node,
    label: &str,
    path: &str,
    depth: usize,
    expanded: &BTreeSet<String>,
    rows: &mut Vec<TreeRow>,
) {
    if !node.children.is_empty() {
        let id = format!("group:{path}");
        let open = expanded.contains(&id);
        rows.push(TreeRow {
            id: id.clone(),
            label: label.into(),
            depth,
            count: count(node),
            field: None,
            open,
            last: false,
            guides: Vec::new(),
        });
        if open {
            if let Some(signal) = &node.signal {
                rows.push(TreeRow {
                    id: signal.id.clone(),
                    label: "value".into(),
                    depth: depth + 1,
                    count: signal.count,
                    field: Some(signal.id.clone()),
                    open: false,
                    last: false,
                    guides: Vec::new(),
                });
            }
            for (label, child) in ordered(&node.children) {
                append(
                    child,
                    label,
                    &format!("{path}/{label}"),
                    depth + 1,
                    expanded,
                    rows,
                );
            }
        }
    } else if let Some(signal) = &node.signal {
        rows.push(TreeRow {
            id: signal.id.clone(),
            label: label.into(),
            depth,
            count: signal.count,
            field: Some(signal.id.clone()),
            open: false,
            last: false,
            guides: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> FieldTree {
        FieldTree::new(&[
            SignalInfo {
                id: "gyro/0/xyz[2]".into(),
                count: 42,
            },
            SignalInfo {
                id: "gyro/1/xyz[0]".into(),
                count: 12,
            },
            SignalInfo {
                id: "esc/0/esc[5].rpm".into(),
                count: 99,
            },
        ])
    }
    #[test]
    fn collapsed_topics_expand_instances_and_named_array_fields() {
        let tree = tree();
        let rows = tree.rows("", &BTreeSet::new());
        assert_eq!(
            rows.iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["esc", "gyro.0", "gyro.1"]
        );
        let expanded = BTreeSet::from(["group:gyro.0".into(), "group:gyro.0/xyz".into()]);
        let rows = tree.rows("", &expanded);
        let field = rows
            .iter()
            .find(|row| row.field.as_deref() == Some("gyro/0/xyz[2]"))
            .unwrap();
        assert_eq!(field.label, "[2]");
        assert_eq!(field.count, 42);
        assert_eq!(field.depth, 2);
    }
    #[test]
    fn leaf_search_keeps_ancestors_and_manual_expansion() {
        let tree = tree();
        let mut expanded = BTreeSet::new();
        let collapsed = tree.rows("RPM", &expanded);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].label, "esc");
        assert!(!collapsed[0].open);
        assert!(collapsed[0].field.is_none());
        for id in ["group:esc", "group:esc/esc", "group:esc/esc/[5]"] {
            expanded.insert(id.into());
        }
        let saved = expanded.clone();
        let rows = tree.rows("RPM", &expanded);
        assert_eq!(
            rows.iter()
                .filter_map(|row| row.field.as_deref())
                .collect::<Vec<_>>(),
            ["esc/0/esc[5].rpm"]
        );
        assert_eq!(rows.last().unwrap().depth, 3);
        assert_eq!(expanded, saved);
        assert_eq!(
            tree.rows("", &expanded)
                .iter()
                .filter(|row| row.depth == 0)
                .count(),
            3
        );
        assert!(tree.rows("missing", &expanded).is_empty());
    }
    #[test]
    fn branch_search_keeps_whole_subtree_and_filters_other_branches() {
        let tree = tree();
        let expanded = BTreeSet::from([
            "group:gyro.0".into(),
            "group:gyro.0/xyz".into(),
            "group:gyro.1".into(),
            "group:gyro.1/xyz".into(),
        ]);
        for query in ["gyro", "xyz"] {
            let rows = tree.rows(query, &expanded);
            assert_eq!(
                rows.iter()
                    .filter_map(|row| row.field.as_deref())
                    .collect::<Vec<_>>(),
                ["gyro/0/xyz[2]", "gyro/1/xyz[0]"]
            );
            assert!(
                rows.iter()
                    .all(|row| row.id.starts_with("group:gyro") || row.id.starts_with("gyro/"))
            );
        }
        let rows = tree.rows("gyro.1", &expanded);
        assert_eq!(
            rows.iter()
                .filter_map(|row| row.field.as_deref())
                .collect::<Vec<_>>(),
            ["gyro/1/xyz[0]"]
        );
        let collapsed = tree.rows("gyro", &BTreeSet::new());
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].count, 1);
        assert!(!collapsed[0].open);
    }
    #[test]
    fn instances_arrays_and_search_results_use_numeric_order() {
        let signals = [10, 2, 1, 0, 11].map(|index| SignalInfo {
            id: format!("topic/{index}/field[{index}]"),
            count: 1,
        });
        let tree = FieldTree::new(&signals);
        let expanded = BTreeSet::from(["group:topic".into()]);
        assert_eq!(
            tree.rows("", &expanded)
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["topic.0", "topic.1", "topic.2", "topic.10", "topic.11"]
        );
        assert_eq!(
            tree.rows("field", &expanded)
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["topic.0", "topic.1", "topic.2", "topic.10", "topic.11"]
        );
        let mut indices = ["[10]", "[0]", "[11]", "[2]", "[1]"];
        indices.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(indices, ["[0]", "[1]", "[2]", "[10]", "[11]"]);
    }
    #[test]
    fn connectors_continue_ancestor_siblings_and_stop_after_filtered_last_child() {
        let expanded = BTreeSet::from(["group:gyro".into(), "group:gyro/xyz".into()]);
        let tree = FieldTree::new(&[
            SignalInfo {
                id: "gyro/0/xyz[2]".into(),
                count: 42,
            },
            SignalInfo {
                id: "gyro/0/other".into(),
                count: 12,
            },
        ]);
        let rows = tree.rows("", &expanded);
        let first = rows.iter().find(|row| row.id == "gyro/0/other").unwrap();
        assert!(!first.last);
        let leaf = rows
            .iter()
            .find(|row| row.field.as_deref() == Some("gyro/0/xyz[2]"))
            .unwrap();
        assert!(leaf.last);
        assert_eq!(
            leaf.guides,
            Vec::<usize>::new(),
            "Last field branch leaves no ancestor trunk"
        );
        let filtered = tree.rows("xyz[2]", &expanded);
        assert!(
            filtered.iter().all(|row| row.last && row.guides.is_empty()),
            "Filtered-out siblings must leave no dangling trunks"
        );
        assert!(
            rows.iter()
                .filter(|row| row.depth == 0)
                .all(|row| row.guides.is_empty())
        );
    }
}
