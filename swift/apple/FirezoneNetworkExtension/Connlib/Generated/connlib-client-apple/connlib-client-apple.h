// File automatically generated by swift-bridge.
typedef struct WrappedSession WrappedSession;
void __swift_bridge__$WrappedSession$_free(void* self);

void* __swift_bridge__$Vec_WrappedSession$new(void);
void __swift_bridge__$Vec_WrappedSession$drop(void* vec_ptr);
void __swift_bridge__$Vec_WrappedSession$push(void* vec_ptr, void* item_ptr);
void* __swift_bridge__$Vec_WrappedSession$pop(void* vec_ptr);
void* __swift_bridge__$Vec_WrappedSession$get(void* vec_ptr, uintptr_t index);
void* __swift_bridge__$Vec_WrappedSession$get_mut(void* vec_ptr, uintptr_t index);
uintptr_t __swift_bridge__$Vec_WrappedSession$len(void* vec_ptr);
void* __swift_bridge__$Vec_WrappedSession$as_ptr(void* vec_ptr);

struct __private__ResultPtrAndPtr __swift_bridge__$WrappedSession$connect(void* api_url, void* token, void* device_id, void* account_slug, void* device_name_override, void* os_version_override, void* log_dir, void* log_filter, void* callback_handler, void* device_info);
void __swift_bridge__$WrappedSession$reset(void* self);
void __swift_bridge__$WrappedSession$set_dns(void* self, void* dns_servers);
void __swift_bridge__$WrappedSession$set_disabled_resources(void* self, void* disabled_resources);
void __swift_bridge__$WrappedSession$disconnect(void* self);


