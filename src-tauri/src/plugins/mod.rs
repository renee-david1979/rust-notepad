//! Process-based plugin host: each plugin is an external program speaking
//! JSON-RPC 2.0 over stdin/stdout, one request/response object per line.

pub mod host;
pub mod manager;

pub use host::{load_manifest, PluginHost, PluginManifest, RpcError, RpcRequest, RpcResponse};
pub use manager::{PluginInfo, PluginManager};
