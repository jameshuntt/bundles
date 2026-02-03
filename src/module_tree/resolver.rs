use super::{hashing::hash_file, node::{ModuleNode, NodeKind}};
use std::fs;
use std::path::{Path, PathBuf};
use syn::{File, Item, ItemMod};

pub fn resolve_module_tree(entry: &Path) -> Vec<ModuleNode> {
    let mut nodes = Vec::new();
    let base_dir = entry.parent().unwrap();
    let content = fs::read_to_string(entry).unwrap_or_default();
    let ast: File = syn::parse_file(&content).unwrap_or_else(|_| panic!("Failed to parse {:?}", entry));

    for item in ast.items {
        if let Item::Mod(ItemMod { ident, content: None, .. }) = item {
            let mod_name = ident.to_string();
            let file_path = base_dir.join(format!("{mod_name}.rs"));
            let mod_rs_path = base_dir.join(&mod_name).join("mod.rs");

            let (kind, path) = match (file_path.exists(), mod_rs_path.exists()) {
                (true, false) => (NodeKind::Module, file_path.clone()),
                (false, true) => (NodeKind::FolderModule, mod_rs_path.clone()),
                (true, true) => (NodeKind::HybridModule, file_path.clone()),
                _ => continue,
            };

            let hash = hash_file(&path);
            let children = resolve_module_tree(&path);

            nodes.push(ModuleNode {
                name: mod_name,
                kind,
                file_path: path,
                hash,
                children,
            });
        }
    }

    nodes
}

pub fn flatten_tree(nodes: &[ModuleNode], list: &mut Vec<PathBuf>) {
    for node in nodes {
        list.push(node.file_path.clone());
        flatten_tree(&node.children, list);
    }
}