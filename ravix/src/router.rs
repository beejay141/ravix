use std::sync::Arc;

use axum::Router;

use crate::{
    container::ContainerRef,
    handler::RouteDescriptor,
    middleware::{apply_cors, CorsConfig},
};

/// Assembles an `axum::Router` from every [`RouteDescriptor`] that was
/// submitted via `inventory::submit!` (i.e. from `#[controller]` macros).
pub struct RouterBuilder;

impl RouterBuilder {
    /// Iterate all registered route descriptors and merge them into one router
    /// backed by the provided DI container as shared state.
    pub fn build(container: ContainerRef) -> Router {
        Self::build_with_cors(container, None, None)
    }

    /// Build the router with optional CORS configuration.
    pub fn build_with_cors(
        container: ContainerRef,
        cors: Option<Arc<CorsConfig>>,
        global_base: Option<String>,
    ) -> Router {
        let mut router: Router<ContainerRef> = Router::new();

        for descriptor in inventory::iter::<RouteDescriptor>() {
            let controller_base = if let Some(ref gb) = global_base {
                Self::join_paths(gb, descriptor.base_path)
            } else {
                descriptor.base_path.to_string()
            };
            let full_path = Self::join_paths(&controller_base, descriptor.path);
            let method_router = (descriptor.handler)(&container);
            router = router.route(&full_path, method_router);
        }

        let router = router.with_state(container);
        apply_cors(router, cors)
    }

    /// Apply CORS configuration to an already-built router.
    pub fn apply_cors_to_router(router: Router, cors: Option<Arc<CorsConfig>>) -> Router {
        apply_cors(router, cors)
    }

    /// Join a controller base path and a handler-local path into a single,
    /// normalised axum route path.
    ///
    /// Examples:
    /// - `/users` + `/`    → `/users`
    /// - `/users` + `/:id` → `/users/:id`
    /// - `/`     + `/ping` → `/ping`
    pub(crate) fn join_paths(base: &str, route: &str) -> String {
        let base = base.trim_end_matches('/');
        let route = route.trim_start_matches('/');
        let joined = if route.is_empty() {
            base.to_string()
        } else {
            format!("{}/{}", base, route)
        };
        // Guarantee the path always begins with '/'
        if joined.starts_with('/') {
            joined
        } else {
            format!("/{}", joined)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RouterBuilder;

    #[test]
    fn join_base_and_empty_route() {
        assert_eq!(RouterBuilder::join_paths("/users", "/"), "/users");
    }

    #[test]
    fn join_base_and_param_route() {
        assert_eq!(RouterBuilder::join_paths("/users", "/:id"), "/users/:id");
    }

    #[test]
    fn join_root_base_and_sub_route() {
        assert_eq!(RouterBuilder::join_paths("/", "/health"), "/health");
    }

    #[test]
    fn join_nested_base() {
        assert_eq!(
            RouterBuilder::join_paths("/api/v1", "/users"),
            "/api/v1/users"
        );
    }

    #[test]
    fn join_global_base_and_controller_base() {
        // Simulates: global_base="/my_service/v1", controller base="/users"
        let controller_base = RouterBuilder::join_paths("/my_service/v1", "/users");
        assert_eq!(controller_base, "/my_service/v1/users");
    }

    #[test]
    fn join_global_base_with_root_controller() {
        // Simulates: global_base="/api", controller base="/"
        let controller_base = RouterBuilder::join_paths("/api", "/");
        assert_eq!(controller_base, "/api");
    }

    #[test]
    fn join_global_base_with_param_route() {
        // Full chain: global_base="/v1", controller_base="/users", route="/:id"
        let controller_base = RouterBuilder::join_paths("/v1", "/users");
        let full = RouterBuilder::join_paths(&controller_base, "/:id");
        assert_eq!(full, "/v1/users/:id");
    }
}
