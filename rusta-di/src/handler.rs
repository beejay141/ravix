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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_descriptor_has_correct_fields() {
        // Test that RouteDescriptor can be constructed with valid values
        let desc = RouteDescriptor {
            method: "GET",
            base_path: "/api",
            path: "/users",
            handler: |_| axum::routing::get(|| async {}),
        };
        
        assert_eq!(desc.method, "GET");
        assert_eq!(desc.base_path, "/api");
        assert_eq!(desc.path, "/users");
    }

    #[test]
    fn route_descriptor_post_method() {
        let desc = RouteDescriptor {
            method: "POST",
            base_path: "/v1",
            path: "/items",
            handler: |_| axum::routing::post(|| async {}),
        };
        
        assert_eq!(desc.method, "POST");
    }

    #[test]
    fn route_descriptor_delete_method() {
        let desc = RouteDescriptor {
            method: "DELETE",
            base_path: "",
            path: "/resource",
            handler: |_| axum::routing::delete(|| async {}),
        };
        
        assert_eq!(desc.method, "DELETE");
        assert_eq!(desc.base_path, "");
    }

    #[test]
    fn route_descriptor_put_method() {
        let desc = RouteDescriptor {
            method: "PUT",
            base_path: "/admin",
            path: "/settings",
            handler: |_| axum::routing::put(|| async {}),
        };
        
        assert_eq!(desc.method, "PUT");
    }

    #[test]
    fn route_descriptor_patch_method() {
        let desc = RouteDescriptor {
            method: "PATCH",
            base_path: "/api",
            path: "/update",
            handler: |_| axum::routing::patch(|| async {}),
        };
        
        assert_eq!(desc.method, "PATCH");
    }
}