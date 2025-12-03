//! MLS Ratchet Tree implementation
//!
//! The ratchet tree is a left-balanced binary tree where:
//! - Leaves represent group members (odd indices: 0, 2, 4, ...)
//! - Parent nodes derive secrets (even indices: 1, 3, 5, ...)
//! - Each node has a public key and optional private key
//! - Path secrets flow from leaf to root for key updates
//!
//! Tree properties:
//! - Size N members → 2N-1 total nodes
//! - Height: O(log N)
//! - Path length from leaf to root: O(log N)

use super::errors::{MlsError, MlsResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Node index in the tree (0-based)
pub type NodeIndex = u32;

/// Leaf index (member position, even numbers only)
pub type LeafIndex = u32;

/// Node in the ratchet tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    /// Node index in the tree
    pub index: NodeIndex,
    /// Public key bytes (HPKE public key)
    pub public_key: Option<Vec<u8>>,
    /// Hash of this node's public key
    pub node_hash: Option<Vec<u8>>,
    /// Parent hash (hash of left || right || public_key)
    pub parent_hash: Option<Vec<u8>>,
}

impl TreeNode {
    /// Create a new empty node
    pub fn new(index: NodeIndex) -> Self {
        Self { index, public_key: None, node_hash: None, parent_hash: None }
    }

    /// Create a leaf node with public key
    pub fn new_leaf(index: NodeIndex, public_key: Vec<u8>) -> Self {
        let node_hash = Some(hash_public_key(&public_key));
        Self { index, public_key: Some(public_key), node_hash, parent_hash: None }
    }

    /// Check if node is blank (no public key)
    pub fn is_blank(&self) -> bool {
        self.public_key.is_none()
    }

    /// Check if this is a leaf node (even index)
    pub fn is_leaf(&self) -> bool {
        self.index % 2 == 0
    }
}

/// MLS Ratchet Tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlsTree {
    /// All nodes in the tree (index → node)
    nodes: HashMap<NodeIndex, TreeNode>,
    /// Number of leaves (members)
    leaf_count: u32,
}

impl MlsTree {
    /// Create a new empty tree
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), leaf_count: 0 }
    }

    /// Get total node count (leaves + parents)
    pub fn size(&self) -> usize {
        if self.leaf_count == 0 {
            0
        } else {
            (2 * self.leaf_count - 1) as usize
        }
    }

    /// Get number of leaves (members)
    pub fn leaf_count(&self) -> u32 {
        self.leaf_count
    }

    /// Convert leaf index to node index (0→0, 1→2, 2→4, ...)
    pub fn leaf_to_node_index(leaf_idx: LeafIndex) -> NodeIndex {
        leaf_idx * 2
    }

    /// Convert node index to leaf index if it's a leaf
    pub fn node_to_leaf_index(node_idx: NodeIndex) -> Option<LeafIndex> {
        if node_idx % 2 == 0 {
            Some(node_idx / 2)
        } else {
            None
        }
    }

    /// Get parent index of a node
    pub fn parent(node_idx: NodeIndex, tree_size: usize) -> Option<NodeIndex> {
        if node_idx as usize >= tree_size {
            return None;
        }

        // Root has no parent
        let root_idx = (tree_size - 1) as NodeIndex;
        if node_idx == root_idx {
            return None;
        }

        // Parent is at (node_idx | 0x01) + 1
        // This works for left-balanced binary trees
        let parent = ((node_idx | 1) + 1) & !1;
        if (parent as usize) < tree_size {
            Some(parent + 1)
        } else {
            // Direct path to root
            Some(root_idx)
        }
    }

    /// Get left child of a parent node
    pub fn left_child(parent_idx: NodeIndex) -> Option<NodeIndex> {
        if parent_idx % 2 == 0 {
            // Not a parent node
            return None;
        }

        // Left child is at parent_idx - ((parent_idx + 1) / 2)
        let offset = (parent_idx + 1) / 2;
        if offset > parent_idx {
            None
        } else {
            Some(parent_idx - offset)
        }
    }

    /// Get right child of a parent node
    pub fn right_child(parent_idx: NodeIndex) -> Option<NodeIndex> {
        if parent_idx % 2 == 0 {
            // Not a parent node
            return None;
        }

        // Right child is at parent_idx + ((parent_idx + 1) / 2)
        let offset = (parent_idx + 1) / 2;
        Some(parent_idx + offset)
    }

    /// Get the root node index
    pub fn root_index(&self) -> Option<NodeIndex> {
        if self.leaf_count == 0 {
            None
        } else {
            // Find the maximum node index (which is the root)
            self.nodes.keys().max().copied()
        }
    }

    /// Add a new leaf node (member joins)
    pub fn add_leaf(&mut self, public_key: Vec<u8>) -> MlsResult<LeafIndex> {
        let leaf_idx = self.leaf_count;
        let node_idx = Self::leaf_to_node_index(leaf_idx);

        let node = TreeNode::new_leaf(node_idx, public_key);
        self.nodes.insert(node_idx, node);
        self.leaf_count += 1;

        // Update parent hashes along the path
        self.update_parent_hashes(node_idx)?;

        Ok(leaf_idx)
    }

    /// Remove a leaf node (member leaves) - blanks the node
    pub fn remove_leaf(&mut self, leaf_idx: LeafIndex) -> MlsResult<()> {
        let node_idx = Self::leaf_to_node_index(leaf_idx);

        if !self.nodes.contains_key(&node_idx) {
            return Err(MlsError::InvalidState(format!("Leaf {} not found", leaf_idx)));
        }

        // Blank the node (remove keys but keep structure)
        let node = TreeNode::new(node_idx);
        self.nodes.insert(node_idx, node);

        // Update parent hashes
        self.update_parent_hashes(node_idx)?;

        Ok(())
    }

    /// Update a leaf node's public key
    pub fn update_leaf(&mut self, leaf_idx: LeafIndex, new_public_key: Vec<u8>) -> MlsResult<()> {
        let node_idx = Self::leaf_to_node_index(leaf_idx);

        if leaf_idx >= self.leaf_count {
            return Err(MlsError::InvalidState(format!("Leaf {} out of bounds", leaf_idx)));
        }

        let node = TreeNode::new_leaf(node_idx, new_public_key);
        self.nodes.insert(node_idx, node);

        // Update parent hashes
        self.update_parent_hashes(node_idx)?;

        Ok(())
    }

    /// Get a node by index
    pub fn get_node(&self, node_idx: NodeIndex) -> Option<&TreeNode> {
        self.nodes.get(&node_idx)
    }

    /// Get direct path from leaf to root (excludes leaf itself)
    pub fn direct_path(&self, leaf_idx: LeafIndex) -> Vec<NodeIndex> {
        let mut path = Vec::new();
        let mut current = Self::leaf_to_node_index(leaf_idx);
        let tree_size = self.size();

        while let Some(parent_idx) = Self::parent(current, tree_size) {
            path.push(parent_idx);
            current = parent_idx;
        }

        path
    }

    /// Compute parent hash for a parent node
    fn compute_parent_hash(&self, parent_idx: NodeIndex) -> Option<Vec<u8>> {
        let left_idx = Self::left_child(parent_idx)?;
        let right_idx = Self::right_child(parent_idx)?;

        let left_hash = self
            .nodes
            .get(&left_idx)
            .and_then(|n| n.node_hash.clone())
            .unwrap_or_else(|| vec![0u8; 32]); // Blank node hash

        let right_hash = self
            .nodes
            .get(&right_idx)
            .and_then(|n| n.node_hash.clone())
            .unwrap_or_else(|| vec![0u8; 32]); // Blank node hash

        // Parent node might not exist yet (blank parent)
        let parent_key_hash = self
            .nodes
            .get(&parent_idx)
            .and_then(|n| n.public_key.as_ref().map(|pk| hash_public_key(pk)))
            .unwrap_or_else(|| vec![0u8; 32]);

        // parent_hash = H(left_hash || right_hash || parent_key_hash)
        let mut hasher = Sha256::new();
        hasher.update(&left_hash);
        hasher.update(&right_hash);
        hasher.update(&parent_key_hash);
        Some(hasher.finalize().to_vec())
    }

    /// Update parent hashes along the path from node to root
    fn update_parent_hashes(&mut self, start_node: NodeIndex) -> MlsResult<()> {
        let tree_size = self.size();
        let mut current = start_node;

        // Update node's own hash
        if let Some(node) = self.nodes.get_mut(&current) {
            if let Some(ref pk) = node.public_key {
                node.node_hash = Some(hash_public_key(pk));
            } else {
                node.node_hash = Some(vec![0u8; 32]); // Blank node
            }
        }

        // Walk up to root
        while let Some(parent_idx) = Self::parent(current, tree_size) {
            let parent_hash = self.compute_parent_hash(parent_idx);

            if let Some(parent_node) = self.nodes.get_mut(&parent_idx) {
                parent_node.parent_hash = parent_hash.clone();
                parent_node.node_hash = parent_hash; // For parent nodes, node_hash = parent_hash
            } else {
                // Create blank parent node
                let mut parent_node = TreeNode::new(parent_idx);
                parent_node.parent_hash = parent_hash.clone();
                parent_node.node_hash = parent_hash;
                self.nodes.insert(parent_idx, parent_node);
            }

            current = parent_idx;
        }

        Ok(())
    }

    /// Compute root hash (for group authentication)
    pub fn root_hash(&self) -> Option<Vec<u8>> {
        let root_idx = self.root_index()?;
        self.nodes.get(&root_idx).and_then(|n| n.node_hash.clone())
    }

    /// Generate path secrets for HPKE encryption (placeholder)
    /// In real MLS, this would derive secrets from leaf to root
    pub fn generate_path_secrets(&self, leaf_idx: LeafIndex) -> MlsResult<Vec<Vec<u8>>> {
        if leaf_idx >= self.leaf_count {
            return Err(MlsError::InvalidState(format!("Leaf {} out of bounds", leaf_idx)));
        }

        let path = self.direct_path(leaf_idx);
        let mut secrets = Vec::new();

        // For each node in the path, derive a secret (placeholder)
        // Real implementation would use HPKE KDF
        for &node_idx in &path {
            let secret = derive_node_secret(node_idx);
            secrets.push(secret);
        }

        Ok(secrets)
    }

    /// Export public tree (for Welcome messages)
    pub fn export_public_nodes(&self) -> Vec<(NodeIndex, Vec<u8>)> {
        self.nodes
            .iter()
            .filter_map(|(idx, node)| node.public_key.as_ref().map(|pk| (*idx, pk.clone())))
            .collect()
    }
}

impl Default for MlsTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash a public key to get node hash
fn hash_public_key(public_key: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"MLS 1.0 node hash");
    hasher.update(public_key);
    hasher.finalize().to_vec()
}

/// Derive a path secret for a node (placeholder - real impl uses HPKE)
fn derive_node_secret(node_idx: NodeIndex) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"MLS 1.0 path secret");
    hasher.update(&node_idx.to_le_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let tree = MlsTree::new();
        assert_eq!(tree.leaf_count(), 0);
        assert_eq!(tree.size(), 0);
        assert_eq!(tree.root_index(), None);
        assert_eq!(tree.root_hash(), None);
    }

    #[test]
    fn test_leaf_node_conversion() {
        assert_eq!(MlsTree::leaf_to_node_index(0), 0);
        assert_eq!(MlsTree::leaf_to_node_index(1), 2);
        assert_eq!(MlsTree::leaf_to_node_index(2), 4);

        assert_eq!(MlsTree::node_to_leaf_index(0), Some(0));
        assert_eq!(MlsTree::node_to_leaf_index(2), Some(1));
        assert_eq!(MlsTree::node_to_leaf_index(1), None); // Parent node
    }

    #[test]
    fn test_add_single_leaf() {
        let mut tree = MlsTree::new();
        let public_key = b"alice_public_key".to_vec();

        let leaf_idx = tree.add_leaf(public_key.clone()).unwrap();

        assert_eq!(leaf_idx, 0);
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.size(), 1);

        let node = tree.get_node(0).unwrap();
        assert_eq!(node.public_key.as_ref().unwrap(), &public_key);
        assert!(node.node_hash.is_some());
    }

    #[test]
    fn test_add_multiple_leaves() {
        let mut tree = MlsTree::new();

        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();
        tree.add_leaf(b"charlie".to_vec()).unwrap();

        assert_eq!(tree.leaf_count(), 3);
        assert_eq!(tree.size(), 5); // 3 leaves + 2 parents

        // Check leaves exist
        assert!(tree.get_node(0).is_some()); // alice
        assert!(tree.get_node(2).is_some()); // bob
        assert!(tree.get_node(4).is_some()); // charlie
    }

    #[test]
    fn test_remove_leaf() {
        let mut tree = MlsTree::new();

        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();

        assert_eq!(tree.leaf_count(), 2);

        // Remove alice
        tree.remove_leaf(0).unwrap();

        assert_eq!(tree.leaf_count(), 2); // Count doesn't change

        let node = tree.get_node(0).unwrap();
        assert!(node.is_blank()); // Node blanked
    }

    #[test]
    fn test_update_leaf() {
        let mut tree = MlsTree::new();

        tree.add_leaf(b"alice_v1".to_vec()).unwrap();

        let old_hash = tree.root_hash().unwrap();

        // Update alice's key
        tree.update_leaf(0, b"alice_v2".to_vec()).unwrap();

        let new_hash = tree.root_hash().unwrap();
        assert_ne!(old_hash, new_hash); // Root hash should change

        let node = tree.get_node(0).unwrap();
        assert_eq!(node.public_key.as_ref().unwrap(), b"alice_v2");
    }

    #[test]
    fn test_direct_path_single_node() {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();

        let path = tree.direct_path(0);
        assert_eq!(path.len(), 0); // Single node, no parents
    }

    #[test]
    fn test_direct_path_two_nodes() {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();

        let path = tree.direct_path(0);
        assert!(path.len() > 0); // Should have parent nodes
    }

    #[test]
    fn test_root_hash_deterministic() {
        let mut tree1 = MlsTree::new();
        tree1.add_leaf(b"alice".to_vec()).unwrap();
        tree1.add_leaf(b"bob".to_vec()).unwrap();

        let mut tree2 = MlsTree::new();
        tree2.add_leaf(b"alice".to_vec()).unwrap();
        tree2.add_leaf(b"bob".to_vec()).unwrap();

        assert_eq!(tree1.root_hash(), tree2.root_hash());
    }

    #[test]
    fn test_root_hash_changes_on_update() {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();

        let hash1 = tree.root_hash().unwrap();

        tree.update_leaf(0, b"alice_updated".to_vec()).unwrap();

        let hash2 = tree.root_hash().unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_generate_path_secrets() {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();
        tree.add_leaf(b"charlie".to_vec()).unwrap();

        let secrets = tree.generate_path_secrets(0).unwrap();
        assert!(!secrets.is_empty());

        // Each secret should be 32 bytes (SHA-256)
        for secret in secrets {
            assert_eq!(secret.len(), 32);
        }
    }

    #[test]
    fn test_generate_path_secrets_invalid_leaf() {
        let tree = MlsTree::new();

        let result = tree.generate_path_secrets(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_export_public_nodes() {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();

        let public_nodes = tree.export_public_nodes();
        assert_eq!(public_nodes.len(), 2); // Only leaves with keys

        // Check alice and bob are exported
        let has_alice = public_nodes.iter().any(|(_, pk)| pk == b"alice");
        let has_bob = public_nodes.iter().any(|(_, pk)| pk == b"bob");
        assert!(has_alice);
        assert!(has_bob);
    }

    #[test]
    fn test_node_is_leaf() {
        let node0 = TreeNode::new(0);
        let node1 = TreeNode::new(1);
        let node2 = TreeNode::new(2);

        assert!(node0.is_leaf());
        assert!(!node1.is_leaf());
        assert!(node2.is_leaf());
    }

    #[test]
    fn test_left_right_children() {
        // For a parent at index 1 (first parent in tree)
        assert_eq!(MlsTree::left_child(1), Some(0));
        assert_eq!(MlsTree::right_child(1), Some(2));

        // Leaf nodes have no children
        assert_eq!(MlsTree::left_child(0), None);
        assert_eq!(MlsTree::right_child(0), None);
    }

    #[test]
    fn test_tree_size_growth() {
        let mut tree = MlsTree::new();

        assert_eq!(tree.size(), 0);

        tree.add_leaf(b"1".to_vec()).unwrap();
        assert_eq!(tree.size(), 1); // 1 leaf

        tree.add_leaf(b"2".to_vec()).unwrap();
        assert_eq!(tree.size(), 3); // 2 leaves + 1 parent

        tree.add_leaf(b"3".to_vec()).unwrap();
        assert_eq!(tree.size(), 5); // 3 leaves + 2 parents

        tree.add_leaf(b"4".to_vec()).unwrap();
        assert_eq!(tree.size(), 7); // 4 leaves + 3 parents
    }
}
