use std::collections::BTreeSet;

use crate::brigadier_tree::{BrigadierTree, BrigadierTreeNode, BrigadierTreeNodeChildren};

pub(in crate::command_map) fn handle_execute_command(tree: BrigadierTree) -> BrigadierTree {
    BrigadierTree {
        commands: tree
            .commands
            .into_iter()
            .map(|command| {
                if command.get_name() == "execute" {
                    handle_execute_command_inner(&command)
                } else {
                    command
                }
            })
            .collect(),
    }
}

fn handle_execute_command_inner(node: &BrigadierTreeNode) -> BrigadierTreeNode {
    let new_children: BrigadierTreeNodeChildren = match node.get_children_or_redirect() {
        BrigadierTreeNodeChildren::Nodes(nodes) => BrigadierTreeNodeChildren::Nodes(
            nodes
                .iter()
                .map(|child| handle_execute_command_inner(child))
                .collect(),
        ),
        BrigadierTreeNodeChildren::Redirect(path) => {
            let mut new_nodes: BTreeSet<BrigadierTreeNode> = BTreeSet::new();
            if path.first().unwrap() == "execute" {
                new_nodes.insert(BrigadierTreeNode::Literal {
                    name: String::from("run"),
                    executable: false,
                    children: BrigadierTreeNodeChildren::Nodes(BTreeSet::new()),
                });
            }
            BrigadierTreeNodeChildren::Nodes(new_nodes)
        }
    };
    node.clone(new_children)
}
