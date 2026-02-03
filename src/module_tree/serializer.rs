use super::node::ModuleNode;
use std::io::{self, Write};

pub fn serialize_tree(node: &ModuleNode, writer: &mut impl Write, depth: usize) -> io::Result<()> {
    let indent = "  ".repeat(depth);
    writeln!(
        writer,
        "{}- {} ({:?}) [{}]",
        indent,
        node.name,
        node.kind,
        node.hash.as_deref().unwrap_or("?")
    )?;
    for child in &node.children {
        serialize_tree(child, writer, depth + 1)?;
    }
    Ok(())
}
