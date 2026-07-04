use std::{
    any::{type_name, Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use axum::extract::FromRef;

use crate::error::FrameworkError;

/// A shared, reference-counted handle to a built [`Container`].
pub type ContainerRef = Arc<Container>;

/// Singleton dependency-injection container.
///
/// Register services with [`Container::register`] and resolve them with
/// [`Container::resolve`]. The type key `T` is typically an `Arc<dyn Trait>`
/// (which is `Sized`, `Clone`, `Send`, `Sync`, and `'static`).
///
/// # Example
/// ```no_run
/// # use ravix::Container;
/// # use std::sync::Arc;
/// let mut c = Container::new();
/// // c.register(Arc::new(InMemoryUserRepository::new()) as Arc<dyn UserRepository>);
/// // let repo: Arc<dyn UserRepository> = c.resolve();
/// ```
pub struct Container {
    singletons: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    named_singletons: HashMap<(TypeId, &'static str), Box<dyn Any + Send + Sync>>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            singletons: HashMap::new(),
            named_singletons: HashMap::new(),
        }
    }

    /// Register a singleton value for type key `T`.
    ///
    /// `T` is typically `Arc<dyn MyTrait>`. The concrete instance is stored
    /// and returned (cloned) on every call to `resolve::<T>()`.
    pub fn register<T: Clone + Send + Sync + 'static>(&mut self, instance: T) {
        self.singletons
            .insert(TypeId::of::<T>(), Box::new(instance));
    }

    /// Resolve the registered singleton for type key `T`.
    ///
    /// # Panics
    /// Panics with a descriptive message when no binding exists for `T`.
    pub fn resolve<T: Clone + Send + Sync + 'static>(&self) -> T {
        let type_id = TypeId::of::<T>();
        self.singletons
            .get(&type_id)
            .unwrap_or_else(|| {
                panic!(
                    "[ravix] No binding registered for `{}`. \
                     Call container.register::<{0}>(...) before App::build().",
                    type_name::<T>()
                )
            })
            .downcast_ref::<T>()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "[ravix] Type mismatch resolving `{}`. This is a bug in the DI container.",
                    type_name::<T>()
                )
            })
    }

    /// Check whether a binding exists for type key `T` without cloning.
    ///
    /// Prefer this over [`try_resolve`] when you only need existence, not the
    /// value. Used internally by [`verify`] to avoid wasteful clones.
    pub fn has_binding<T: 'static>(&self) -> bool {
        self.singletons.contains_key(&TypeId::of::<T>())
    }

    /// Attempt to resolve type `T`. Returns `None` when no binding exists.
    ///
    /// # Example
    /// ```no_run
    /// # use ravix::Container;
    /// # use std::sync::Arc;
    /// let c = Container::new();
    /// let opt: Option<Arc<dyn std::any::Any + Send + Sync>> = c.try_resolve();
    /// assert!(opt.is_none());
    /// ```
    pub fn try_resolve<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        let type_id = TypeId::of::<T>();
        self.singletons
            .get(&type_id)
            .and_then(|boxed| boxed.downcast_ref::<T>().cloned())
    }

    /// Register a singleton value for type key `T` under a name.
    ///
    /// Multiple implementations of the same trait can be registered with
    /// different names and resolved later with [`Container::resolve_named`].
    ///
    /// # Example
    /// ```no_run
    /// # use ravix::Container;
    /// # use std::sync::Arc;
    /// # trait Cache: Send + Sync {}
    /// # struct RedisCache;
    /// # impl Cache for RedisCache {}
    /// # struct MemoryCache;
    /// # impl Cache for MemoryCache {}
    /// let mut c = Container::new();
    /// c.register_named::<Arc<dyn Cache>>("redis", Arc::new(RedisCache));
    /// c.register_named::<Arc<dyn Cache>>("memory", Arc::new(MemoryCache));
    /// ```
    pub fn register_named<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        instance: T,
    ) {
        self.named_singletons
            .insert((TypeId::of::<T>(), name), Box::new(instance));
    }

    /// Resolve a named singleton for type key `T`.
    ///
    /// # Panics
    /// Panics with a descriptive message when no named binding exists for `T`.
    pub fn resolve_named<T: Clone + Send + Sync + 'static>(&self, name: &'static str) -> T {
        let type_id = TypeId::of::<T>();
        self.named_singletons
            .get(&(type_id, name))
            .unwrap_or_else(|| {
                panic!(
                    "[ravix] No named binding '{}' registered for `{}`.",
                    name,
                    type_name::<T>()
                )
            })
            .downcast_ref::<T>()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "[ravix] Type mismatch resolving named '{}' for `{}`.",
                    name,
                    type_name::<T>()
                )
            })
    }

    /// Attempt to resolve a named singleton. Returns `None` when no binding
    /// exists under the given name.
    pub fn try_resolve_named<T: Clone + Send + Sync + 'static>(
        &self,
        name: &'static str,
    ) -> Option<T> {
        let type_id = TypeId::of::<T>();
        self.named_singletons
            .get(&(type_id, name))
            .and_then(|boxed| boxed.downcast_ref::<T>().cloned())
    }

    /// Run all registered binding checks. Returns a list of missing-binding errors.
    ///
    /// Each `#[injectable]` type automatically submits a [`BindingCheck`] for
    /// every required (non-optional) `#[inject]` field via `inventory`.  Call
    /// this once after all registrations are done to catch missing bindings
    /// at startup.
    pub fn verify(&self) -> Vec<String> {
        inventory::iter::<BindingCheck>()
            .filter_map(|bc| (bc.check)(self).err())
            .collect()
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait implemented by `#[injectable]` structs.
///
/// The proc-macro generates an implementation that resolves each
/// `#[inject]`-annotated field from the container and constructs the type.
pub trait Injectable: Sized + Send + Sync + 'static {
    fn construct(container: &Container) -> Arc<Self>;
}

/// Axum extractor that resolves a registered `T` from [`ContainerRef`] state.
///
/// `T` is typically `Arc<dyn MyTrait>` — an owned, cloneable handle to a
/// service registered with [`Container::register`].
///
/// # Example
/// ```no_run
/// # use ravix::{Inject, Response};
/// # use std::sync::Arc;
/// // async fn handler(Inject(svc): Inject<Arc<dyn UserService>>) -> impl IntoResponse {
/// //     Response::json(svc.find_all().await)
/// // }
/// ```
pub struct Inject<T>(pub T);

#[async_trait::async_trait]
impl<T, S> axum::extract::FromRequestParts<S> for Inject<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
    ContainerRef: FromRef<S>,
{
    type Rejection = FrameworkError;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let container = ContainerRef::from_ref(state);
        Ok(Inject(container.resolve::<T>()))
    }
}

// ---------------------------------------------------------------------------
// Binding verification — submitted by #[injectable] for required fields
// ---------------------------------------------------------------------------

/// A verification check submitted by `#[injectable]` via inventory.
pub struct BindingCheck {
    /// Human-readable type name (for error messages).
    pub type_name: &'static str,
    /// Returns `Ok(())` if the type can be resolved, `Err(msg)` otherwise.
    pub check: fn(&Container) -> Result<(), String>,
}

// SAFETY: fn pointers are always Send + Sync; `&'static str` is too.
unsafe impl Send for BindingCheck {}
unsafe impl Sync for BindingCheck {}

inventory::collect!(BindingCheck);

#[cfg(test)]
mod tests {
    use super::*;

    trait Greeter: Send + Sync {
        fn greet(&self) -> &'static str;
    }

    struct HelloGreeter;
    impl Greeter for HelloGreeter {
        fn greet(&self) -> &'static str {
            "hello"
        }
    }

    #[test]
    fn bind_and_resolve_roundtrip() {
        let mut c = Container::new();
        c.register(Arc::new(HelloGreeter) as Arc<dyn Greeter>);
        let g: Arc<dyn Greeter> = c.resolve();
        assert_eq!(g.greet(), "hello");
    }

    #[test]
    fn resolve_returns_same_arc() {
        let mut c = Container::new();
        c.register(Arc::new(HelloGreeter) as Arc<dyn Greeter>);
        let a: Arc<dyn Greeter> = c.resolve();
        let b: Arc<dyn Greeter> = c.resolve();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    #[should_panic(expected = "No binding registered")]
    fn resolve_missing_panics() {
        let c = Container::new();
        let _: Arc<dyn Greeter> = c.resolve();
    }

    #[test]
    fn try_resolve_returns_none_for_missing() {
        let c = Container::new();
        assert!(c.try_resolve::<Arc<dyn Greeter>>().is_none());
    }

    #[test]
    fn try_resolve_returns_some_when_registered() {
        let mut c = Container::new();
        c.register(Arc::new(HelloGreeter) as Arc<dyn Greeter>);
        assert!(c.try_resolve::<Arc<dyn Greeter>>().is_some());
    }

    #[test]
    fn register_named_and_resolve() {
        let mut c = Container::new();
        c.register_named::<Arc<dyn Greeter>>("hello", Arc::new(HelloGreeter));
        let g: Arc<dyn Greeter> = c.resolve_named("hello");
        assert_eq!(g.greet(), "hello");
    }

    #[test]
    fn try_resolve_named_returns_none_for_missing_name() {
        let c = Container::new();
        assert!(c.try_resolve_named::<Arc<dyn Greeter>>("bogus").is_none());
    }

    #[test]
    fn try_resolve_named_returns_some_when_registered() {
        let mut c = Container::new();
        c.register_named::<Arc<dyn Greeter>>("hello", Arc::new(HelloGreeter));
        assert!(c.try_resolve_named::<Arc<dyn Greeter>>("hello").is_some());
    }

    #[test]
    fn verify_returns_empty_when_no_checks_registered() {
        let c = Container::new();
        let errors = c.verify();
        assert!(errors.is_empty());
    }
}
