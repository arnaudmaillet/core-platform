mod common;

use std::sync::Arc;

use auth_context::{
    current_principal, inject_into_span, with_principal, CurrentPrincipal, Permission, PrincipalId,
};

fn make_principal(user_id: &str, tenant_id: Option<&str>, perms: &[&str]) -> CurrentPrincipal<()> {
    CurrentPrincipal {
        user_id: PrincipalId::new(user_id),
        tenant_id: tenant_id.map(str::to_owned),
        permissions: perms.iter().map(|p| Permission::new(*p)).collect(),
        raw_claims: (),
    }
}

#[tokio::test]
async fn current_principal_is_none_outside_scope() {
    assert!(
        current_principal().is_none(),
        "current_principal() must return None before with_principal() is called"
    );
}

#[tokio::test]
async fn principal_is_accessible_inside_scope() {
    let principal = Arc::new(make_principal("user-1", None, &["openid"]));

    with_principal(Arc::clone(&principal), async {
        let stored = current_principal().expect("principal must be Some inside with_principal()");
        assert_eq!(stored.user_id().as_str(), "user-1");
        assert_eq!(stored.permissions()[0].as_str(), "openid");
    })
    .await;
}

#[tokio::test]
async fn principal_available_across_await_points() {
    let principal = Arc::new(make_principal("user-2", Some("tenant-x"), &["posts:write"]));

    with_principal(Arc::clone(&principal), async {
        tokio::task::yield_now().await;

        let stored = current_principal().unwrap();
        assert_eq!(stored.user_id().as_str(), "user-2");
        assert_eq!(stored.tenant_id(), Some("tenant-x"));

        tokio::task::yield_now().await;

        let stored2 = current_principal().unwrap();
        assert!(stored2.permissions().iter().any(|p| p.as_str() == "posts:write"));
    })
    .await;
}

#[tokio::test]
async fn principal_is_none_after_scope_exits() {
    let principal = Arc::new(make_principal("user-3", None, &[]));

    with_principal(Arc::clone(&principal), async {
        assert!(current_principal().is_some());
    })
    .await;

    assert!(
        current_principal().is_none(),
        "current_principal() must be None after the with_principal() future completes"
    );
}

#[tokio::test]
async fn nested_with_principal_shadows_outer() {
    let outer = Arc::new(make_principal("user-outer", None, &[]));
    let inner = Arc::new(make_principal("user-inner", None, &[]));

    with_principal(Arc::clone(&outer), async {
        assert_eq!(current_principal().unwrap().user_id().as_str(), "user-outer");

        with_principal(Arc::clone(&inner), async {
            assert_eq!(current_principal().unwrap().user_id().as_str(), "user-inner");
        })
        .await;

        // Outer scope restored after inner exits.
        assert_eq!(current_principal().unwrap().user_id().as_str(), "user-outer");
    })
    .await;
}

#[tokio::test]
async fn inject_into_span_does_not_panic_outside_scope() {
    // Must be a no-op, not a panic, when called outside a with_principal() scope.
    inject_into_span();
}

#[tokio::test]
async fn inject_into_span_does_not_panic_inside_scope() {
    let principal = Arc::new(make_principal("user-span", Some("t-1"), &[]));

    with_principal(Arc::clone(&principal), async {
        // Span is created here; fields may be Empty if not instrumented,
        // but record() must not panic.
        inject_into_span();
    })
    .await;
}

#[tokio::test]
async fn downcast_to_concrete_type_succeeds() {
    let principal = Arc::new(make_principal("user-cast", None, &["admin"]));

    with_principal(Arc::clone(&principal), async {
        let any_p = current_principal().unwrap();
        let concrete = any_p
            .as_any()
            .downcast_ref::<CurrentPrincipal<()>>()
            .expect("downcast to CurrentPrincipal<()> must succeed");

        assert_eq!(concrete.user_id.as_str(), "user-cast");
    })
    .await;
}

/// Verifies that `current_principal()` on a spawned sub-task does NOT
/// inherit the parent task's principal (task-local is per-task, not
/// propagated across `tokio::spawn` boundaries by default).
#[tokio::test]
async fn spawned_task_does_not_inherit_principal() {
    let principal = Arc::new(make_principal("user-parent", None, &[]));

    with_principal(principal, async {
        let child = tokio::spawn(async {
            current_principal()
        });

        let result = child.await.unwrap();
        assert!(
            result.is_none(),
            "a spawned task must not see the parent's task-local principal"
        );
    })
    .await;
}
