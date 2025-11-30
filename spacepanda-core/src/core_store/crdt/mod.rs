/*
    CRDT subsystem - Conflict-Free Replicated Data Types
    
    Implements various CRDT types for distributed state management.
*/

pub mod traits;
pub mod vector_clock;
pub mod lww_register;
pub mod or_set;
pub mod or_map;
pub mod g_list;
pub mod oplog;
pub mod signer;

pub use traits::{Crdt, CrdtOperation, OperationMetadata, TombstoneCrdt, ValidatedCrdt};
pub use vector_clock::{VectorClock, NodeId};
pub use lww_register::{LWWRegister, LWWOperation};
pub use or_set::{ORSet, ORSetOperation, AddId};
pub use or_map::{ORMap, ORMapOperation};
pub use g_list::{GList, GListOperation, ElementId};
pub use oplog::{OpLog, OpLogEntry};
pub use signer::{Signature, PublicKey, SigningKey, OperationSigner, OperationVerifier};
