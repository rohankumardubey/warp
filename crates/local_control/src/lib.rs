pub mod auth;
pub mod client;
pub mod discovery;
pub mod protocol;
pub mod selection;

pub use auth::{
    AuthToken, AuthenticatedUserGrant, CredentialGrant, CredentialRequest, ScopedCredential,
};
pub use discovery::{
    ControlEndpoint, CredentialBrokerReference, InstanceId, InstanceRecord, RegisteredInstance,
    discovery_dir,
};
pub use protocol::{
    Action, ActionImplementationStatus, ActionKind, ActionMetadata, AuthenticatedUserRequirement,
    ControlError, ControlResponse, ErrorCode, ErrorResponseEnvelope, ExecutionContextProof,
    InvocationContext, PROTOCOL_VERSION, PaneSelector, PermissionCategory, RequestEnvelope,
    ResponseEnvelope, RiskTier, StateDataCategory, TabSelector, TargetScope, TargetSelector,
    WindowSelector,
};
