# api-param-order

> Keep the same conceptual parameters in the same order across related functions

## Why It Matters

When `user_id` and `tenant_id` swap places between `create` and `delete`, a caller who copies an earlier call site silently transposes arguments of the same type. The Microsoft Pragmatic Rust Guidelines put call-specific values first, shared context (loggers, clocks) last, and a single closure last of all. One order, used everywhere in the crate, is cheaper to review than a comment on each function.

## Bad

```rust
pub struct Logger;
pub struct TenantId(pub u64);
pub struct UserId(pub u64);

pub fn create_user(logger: &Logger, user_id: UserId, tenant_id: TenantId) -> bool {
    let _ = (logger, user_id, tenant_id);
    true
}

pub fn delete_user(tenant_id: TenantId, user_id: UserId, logger: &Logger) -> bool {
    let _ = (logger, user_id, tenant_id);
    true
}
```

## Good

```rust
pub struct Logger;
pub struct TenantId(pub u64);
pub struct UserId(pub u64);

pub fn create_user(tenant_id: TenantId, user_id: UserId, logger: &Logger) -> bool {
    let _ = (logger, user_id, tenant_id);
    true
}

pub fn delete_user(tenant_id: TenantId, user_id: UserId, logger: &Logger) -> bool {
    let _ = (logger, user_id, tenant_id);
    true
}

pub fn rename_user(
    tenant_id: TenantId,
    user_id: UserId,
    new_name: &str,
    logger: &Logger,
) -> bool {
    let _ = (logger, user_id, tenant_id, new_name);
    true
}

fn main() {
    let logger = Logger;
    let _ = create_user(TenantId(1), UserId(2), &logger);
    let _ = delete_user(TenantId(1), UserId(2), &logger);
    let _ = rename_user(TenantId(1), UserId(2), "ada", &logger);
}
```

## See Also

- [api-newtype-safety](api-newtype-safety.md) - newtypes catch swaps that parameter order alone cannot
- [name-funcs-snake](name-funcs-snake.md) - consistent names make a consistent order easier to scan
- [doc-all-public](doc-all-public.md) - document the shared order once at the module, not on every signature
