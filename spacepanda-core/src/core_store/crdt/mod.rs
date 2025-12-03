/*
    CRDT subsystem - Conflict-Free Replicated Data Types

    Implements various CRDT types for distributed state management.
*/

pub mod g_list;
pub mod lww_register;
pub mod oplog;
pub mod or_map;
pub mod or_set;
pub mod signer;
pub mod traits;
pub mod validated;
pub mod vector_clock;

pub use g_list::{ElementId, GList, GListOperation};
pub use lww_register::{LWWOperation, LWWRegister};
pub use oplog::{OpLog, OpLogEntry};
pub use or_map::{ORMap, ORMapOperation};
pub use or_set::{AddId, ORSet, ORSetOperation};
pub use signer::{OperationSigner, OperationVerifier, PublicKey, Signature, SigningKey};
pub use traits::{
    Crdt, CrdtOperation, OperationMetadata, TombstoneCrdt, ValidatedCrdt as ValidatedCrdtTrait,
};
pub use validated::{HasMetadata, SignatureConfig, SignedOperation, ValidatedCrdt};
pub use vector_clock::{NodeId, VectorClock};
