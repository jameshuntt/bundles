//! A crate's file modules, walked from an entry file.
use crate::{io_at, BundleError};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Item, ItemMod};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// `name.rs`
    Module,
    /// `name/mod.rs`
    FolderModule,
    /// `#[path = "..."]`
    PathAttr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    pub name: String,
    pub kind: NodeKind,
    pub file_path: PathBuf,
    pub sha256: String,
    pub children: Vec<ModuleNode>,
}

/// The tree of file modules declared (transitively) by `entry`.
///
/// Children of `lib.rs`, `main.rs` and `x/mod.rs` live next to the file;
/// children of `x.rs` live in `x/` (2018 layout). Inline `mod y { mod z; }`
/// blocks are followed into `y/`. A file that fails to parse is an error,
/// not a panic; a declared module with no file is skipped.
pub fn resolve_module_tree(entry: &Path) -> Result<Vec<ModuleNode>, BundleError> {
    let content = fs::read_to_string(entry).map_err(io_at(entry))?;
    let ast = syn::parse_file(&content).map_err(|e| BundleError::Parse { path: entry.to_path_buf(), message: e.to_string() })?;
    let dir = entry.parent().unwrap_or(Path::new("."));
    let stem = entry.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let children_dir = if matches!(stem, "lib" | "main" | "mod") { dir.to_path_buf() } else { dir.join(stem) };
    let mut nodes = Vec::new();
    collect(&ast.items, &children_dir, &mut nodes)?;
    Ok(nodes)
}

fn collect(items: &[Item], dir: &Path, nodes: &mut Vec<ModuleNode>) -> Result<(), BundleError> {
    for item in items {
        let Item::Mod(ItemMod { ident, content, attrs, .. }) = item else { continue };
        let name = ident.to_string();
        if let Some((_, inner)) = content {
            collect(inner, &dir.join(&name), nodes)?;
            continue;
        }
        let path_attr = attrs.iter().find(|a| a.path().is_ident("path")).and_then(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value()),
                _ => None,
            },
            _ => None,
        });
        let (kind, file_path) = if let Some(p) = path_attr {
            (NodeKind::PathAttr, dir.join(p))
        } else {
            let flat = dir.join(format!("{name}.rs"));
            let folder = dir.join(&name).join("mod.rs");
            if flat.is_file() {
                (NodeKind::Module, flat)
            } else if folder.is_file() {
                (NodeKind::FolderModule, folder)
            } else {
                continue;
            }
        };
        if !file_path.is_file() {
            continue;
        }
        let bytes = fs::read(&file_path).map_err(io_at(&file_path))?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let children = resolve_module_tree(&file_path)?;
        nodes.push(ModuleNode { name, kind, file_path, sha256, children });
    }
    Ok(())
}

/// Every file in the tree, depth-first, in declaration order.
pub fn flatten_tree(nodes: &[ModuleNode], list: &mut Vec<PathBuf>) {
    for node in nodes {
        list.push(node.file_path.clone());
        flatten_tree(&node.children, list);
    }
}
