use std::{
    any::{type_name, Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use axum::extract::FromRef;

use crate::error::DiError;

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
/// # use rusta_di::Container;
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
                    "[rusta-di] No binding registered for `{}`. \
                     Call container.register::<{0}>(...) before App::build().",
                    type_name::<T>()
                )
            })
            .downcast_ref::<T>()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "[rusta-di] Type mismatch resolving `{}`. This is a bug in the DI container.",
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
    /// # use rusta_di::Container;
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
    /// # use rusta_di::Container;
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
                    "[rusta-di] No named binding '{}' registered for `{}`.",
                    name,
                    type_name::<T>()
                )
            })
            .downcast_ref::<T>()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "[rusta-di] Type mismatch resolving named '{}' for `{}`.",
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
/// # use rusta_di::Inject;
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
    type Rejection = DiError;

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

    struct EnglishGreeter;
    impl Greeter for EnglishGreeter {
        fn greet(&self) -> &'static str {
            "Hello"
        }
    }

    #[test]
    fn test_register_and_resolve() {
        let mut c = Container::new();
        c.register(Arc::new(EnglishGreeter) as Arc<dyn Greeter>);
        let g = c.resolve::<Arc<dyn Greeter>>();
        assert_eq!(g.greet(), "Hello");
    }

    #[test]
    fn test_has_binding_returns_true_when_registered() {
        let mut c = Container::new();
        c.register(Arc::new(EnglishGreeter) as Arc<dyn Greeter>);
        assert!(c.has_binding::<Arc<dyn Greeter>>());
    }

    #[test]
    fn test_has_binding_returns_false_when_missing() {
        let c = Container::new();
        assert!(!c.has_binding::<Arc<dyn Greeter>>());
    }

    #[test]
    fn test_try_resolve_returns_some_when_registered() {
        let mut c = Container::new();
        c.register(Arc::new(EnglishGreeter) as Arc<dyn Greeter>);
        assert!(c.try_resolve::<Arc<dyn Greeter>>().is_some());
    }

    #[test]
    fn test_try_resolve_returns_none_when_missing() {
        let c = Container::new();
        assert!(c.try_resolve::<Arc<dyn Greeter>>().is_none());
    }

    #[test]
    fn test_register_named_and_resolve_named() {
        let mut c = Container::new();
        c.register_named::<Arc<dyn Greeter>>("english", Arc::new(EnglishGreeter));
        let g = c.resolve_named::<Arc<dyn Greeter>>("english");
        assert_eq!(g.greet(), "Hello");
    }

    #[test]
    fn test_try_resolve_named_returns_some_when_registered() {
        let mut c = Container::new();
        c.register_named::<Arc<dyn Greeter>>("english", Arc::new(EnglishGreeter));
        assert!(c.try_resolve_named::<Arc<dyn Greeter>>("english").is_some());
    }

    #[test]
    fn test_try_resolve_named_returns_none_when_missing() {
        let c = Container::new();
        assert!(c.try_resolve_named::<Arc<dyn Greeter>>("missing").is_none());
    }

    #[test]
    fn test_default_container_is_empty() {
        let c = Container::default();
        assert!(c.try_resolve::<Arc<dyn Greeter>>().is_none());
    }

    #[test]
    fn test_verify_returns_empty_when_no_checks() {
        let c = Container::new();
        let errors = c.verify();
        assert!(errors.is_empty());
    }

    #[test]
    #[should_panic(expected = "No binding registered")]
    fn test_resolve_panics_when_missing() {
        let c = Container::new();
        c.resolve::<Arc<dyn Greeter>>();
    }

    #[test]
    #[should_panic(expected = "No named binding")]
    fn test_resolve_named_panics_when_missing() {
        let c = Container::new();
        c.resolve_named::<Arc<dyn Greeter>>("missing");
    }

    #[test]
    fn test_multiple_registrations_same_type_overwrites() {
        let mut c = Container::new();
        
        // Register first greeter
        c.register(Arc::new(EnglishGreeter) as Arc<dyn Greeter>);
        assert_eq!(c.resolve::<Arc<dyn Greeter>>().greet(), "Hello");

        // Overwrite with same concrete type
        c.register(Arc::new(EnglishGreeter) as Arc<dyn Greeter>);
        assert_eq!(c.resolve::<Arc<dyn Greeter>>().greet(), "Hello");
    }

    #[test]
    fn test_multiple_named_registrations_same_type() {
        let mut c = Container::new();
        
        // Register multiple implementations under different names
        c.register_named::<Arc<dyn Greeter>>("first", Arc::new(EnglishGreeter));
        c.register_named::<Arc<dyn Greeter>>("second", Arc::new(EnglishGreeter));

        // Both should be resolvable
        let first = c.resolve_named::<Arc<dyn Greeter>>("first");
        let second = c.resolve_named::<Arc<dyn Greeter>>("second");
        
        assert_eq!(first.greet(), "Hello");
        assert_eq!(second.greet(), "Hello");
    }

    #[test]
    fn test_primitive_type_registration() {
        let mut c = Container::new();
        c.register(42_i32);
        assert_eq!(c.resolve::<i32>(), 42);
    }

    #[test]
    fn test_string_registration() {
        let mut c = Container::new();
        c.register(String::from("test string"));
        assert_eq!(c.resolve::<String>(), "test string");
    }

    #[test]
    fn test_arced_value_registration() {
        let mut c = Container::new();
        let value = Arc::new(String::from("arc value"));
        c.register(value.clone());
        
        let resolved = c.resolve::<Arc<String>>();
        assert_eq!(&*resolved, "arc value");
    }

    #[test]
    fn test_inject_wraps_value() {
        let value = Arc::new(EnglishGreeter) as Arc<dyn Greeter>;
        let inject = Inject(value);
        assert_eq!(inject.0.greet(), "Hello");
    }

    #[test]
    fn test_inject_unwraps_value() {
        let value = Arc::new(EnglishGreeter) as Arc<dyn Greeter>;
        let inject = Inject(value);
        let inner: Arc<dyn Greeter> = inject.0;
        assert_eq!(inner.greet(), "Hello");
    }
}