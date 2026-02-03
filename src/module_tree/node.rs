use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    CrateRoot,
    Module,
    FolderModule,
    LeafFile,
    HybridModule,
    Resource,
}

#[derive(Debug, Clone)]
pub struct ModuleNode {
    pub name: String,
    pub kind: NodeKind,
    pub file_path: PathBuf,
    pub hash: Option<String>,
    pub children: Vec<ModuleNode>,
}
