use std::collections::{BTreeSet, HashMap};

use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode, BrigadierTreeNodeChildren};

pub(in crate::command_map) fn consolidate_literals_into_enums(
    tree: BrigadierTree,
) -> BrigadierTree {
    BrigadierTree {
        commands: tree
            .commands
            .iter()
            .map(|node| consolidate_literals_into_enums_inner(node).0)
            .collect(),
    }
}

fn consolidate_literals_into_enums_inner(node: &BrigadierTreeNode) -> (BrigadierTreeNode, String) {
    let children: &BTreeSet<BrigadierTreeNode> = match node.get_children_or_redirect() {
        BrigadierTreeNodeChildren::Nodes(nodes) => nodes,
        BrigadierTreeNodeChildren::Redirect(path) => {
            return (
                node.clone(BrigadierTreeNodeChildren::Redirect(path.clone())),
                format!("->{}", path.join(",")),
            );
        }
    };
    let mut new_children_and_subtrees: BTreeSet<(BrigadierTreeNode, String)> = BTreeSet::new();
    let mut merge_candidates: HashMap<(String, bool), Vec<BrigadierTreeNode>> = HashMap::new();
    for child in children {
        let (new_child, new_child_subtrees_canon) = consolidate_literals_into_enums_inner(child);
        match new_child {
            BrigadierTreeNode::Literal { .. } => {
                let key: (String, bool) = (new_child_subtrees_canon, new_child.is_executable());
                if !merge_candidates.contains_key(&key) {
                    merge_candidates.insert(key.clone(), Vec::new());
                }
                merge_candidates.get_mut(&key).unwrap().push(new_child);
            }
            _ => {
                new_children_and_subtrees.insert((new_child, new_child_subtrees_canon));
            }
        }
    }
    for ((subtree_canon, is_executable), candidates) in merge_candidates.iter_mut() {
        let subtree_canon: String = subtree_canon.clone();
        if candidates.is_empty() {
            continue;
        } else if candidates.len() == 1 {
            new_children_and_subtrees.insert((candidates.pop().unwrap(), subtree_canon));
        } else {
            let enum_values: Vec<String> =
                candidates.iter().map(|n| n.get_name().clone()).collect();
            let enum_children: BrigadierTreeNodeChildren = match candidates.pop().unwrap() {
                BrigadierTreeNode::Literal { children, .. } => children,
                _ => panic!("Expected merge candidates to all be literals"),
            };
            new_children_and_subtrees.insert((
                BrigadierTreeNode::Enum {
                    name: enum_values.join("|"),
                    values: enum_values,
                    executable: is_executable.clone(),
                    children: enum_children,
                },
                subtree_canon,
            ));
        }
    }
    let mut new_children: BTreeSet<BrigadierTreeNode> = BTreeSet::new();
    let mut subtrees: Vec<String> = Vec::new();
    for (new_child, new_child_subtrees) in new_children_and_subtrees.into_iter() {
        let new_child_canon: String = new_child.get_canonical_string();
        new_children.insert(new_child);
        subtrees.push(format!("{}[{}]", new_child_canon, new_child_subtrees));
    }
    (
        node.clone(BrigadierTreeNodeChildren::Nodes(new_children)),
        subtrees.join(";"),
    )
}
