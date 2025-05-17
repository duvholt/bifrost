pub mod policy;
pub mod serviceid;
pub mod traits;

#[cfg(feature = "manager")]
pub mod error;
#[cfg(feature = "manager")]
pub mod manager;
#[cfg(feature = "manager")]
pub mod rpc;
#[cfg(feature = "manager")]
pub mod runservice;
#[cfg(feature = "manager")]
pub mod template;
