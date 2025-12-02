pub mod onion_router;
pub mod overlay_discovery;
pub mod rate_limiter;
pub mod route_table;
pub mod rpc_protocol;
pub mod router_handle;
pub mod session_manager;
pub mod transport_manager;
pub mod metrics;

#[cfg(test)]
mod tests;

pub use onion_router::{
    InnerEnvelope, OnionCommand, OnionConfig, OnionEvent, OnionHeader, OnionRouter,
};
pub use overlay_discovery::{
    DiscoveryCommand, DiscoveryConfig, DiscoveryEvent, OverlayDiscovery, PeerDescriptor,
    PeerExchangeRequest, PeerExchangeResponse,
};
pub use rate_limiter::{RateLimiter, RateLimiterConfig, RateLimitResult};
pub use route_table::{Capability, GeoLocation, PeerInfo, PeerStats, RouteTable, RouteTableCommand};
pub use rpc_protocol::{RpcCommand, RpcError, RpcMessage, RpcProtocol, RpcRequest};
pub use router_handle::{RouterCommand, RouterEvent, RouterHandle};
pub use session_manager::{PeerId, SessionCommand, SessionEvent, SessionManager};
pub use transport_manager::{TransportCommand, TransportEvent, TransportManager};
