use axum::routing::MethodRouter;

use crate::container::ContainerRef;

/// Describes a single HTTP route registered by the `#[controller]` proc-macro.
///
/// Every `#[get]`, `#[post]`, etc. annotation inside a `#[controller]` block
/// causes the macro to emit an `inventory::submit!(RouteDescriptor { ... })`
/// at the call-site. [`crate::router::RouterBuilder`] then collects all
/// submitted descriptors at startup and assembles the final `axum::Router`.
pub struct RouteDescriptor {
    /// HTTP verb string — "GET", "POST", "PUT", "DELETE", or "PATCH".
    pub method: &'static str,
    /// Controller base path from `#[controller("/base")]`.
    pub base_path: &'static str,
    /// Handler-local path from `#[get("/path")]`.
    pub path: &'static str,
    /// Factory fn called once at startup. Receives the DI container, resolves
    /// the controller singleton, and returns a `MethodRouter` backed by a closure
    /// that captures the pre-built `Arc<Controller>` — eliminating per-request
    /// container lookups.
    pub handler: fn(&ContainerRef) -> MethodRouter<ContainerRef>,
}

// SAFETY: fn pointers are always Send + Sync; `&'static str` is too.
unsafe impl Send for RouteDescriptor {}
unsafe impl Sync for RouteDescriptor {}

inventory::collect!(RouteDescriptor);
