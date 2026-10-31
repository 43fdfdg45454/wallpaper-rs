use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use std::sync::OnceLock;

pub type Ptr = *mut c_void;
pub type Text = *const c_char;
pub type GType = usize;
pub type Callback = unsafe extern "C" fn();

pub const LIBRARY: &str = "libWPEWebKit-2.0.so.1";

macro_rules! api {
    ($($name:ident: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        pub struct Api {
            _lib: libloading::Library,
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
            pub g_object_new: unsafe extern "C" fn(GType, Text, ...) -> Ptr,
            pub g_object_set: unsafe extern "C" fn(Ptr, Text, ...),
        }

        impl Api {
            unsafe fn load(lib: libloading::Library) -> Result<Api, String> {
                let find = |name: &str| unsafe {
                    lib.get::<*const c_void>(name.as_bytes()).map(|s| *s).map_err(|_| name.to_owned())
                };
                unsafe {
                    Ok(Api {
                        $($name: std::mem::transmute::<*const c_void, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(
                            find(stringify!($name))?,
                        ),)*
                        g_object_new: std::mem::transmute::<*const c_void, unsafe extern "C" fn(GType, Text, ...) -> Ptr>(
                            find("g_object_new")?,
                        ),
                        g_object_set: std::mem::transmute::<*const c_void, unsafe extern "C" fn(Ptr, Text, ...)>(
                            find("g_object_set")?,
                        ),
                        _lib: lib,
                    })
                }
            }
        }
    };
}

api! {
    g_main_loop_new: fn(Ptr, c_int) -> Ptr;
    g_main_loop_run: fn(Ptr);
    g_unix_fd_add: fn(c_int, c_uint, Ptr, Ptr) -> c_uint;
    g_timeout_add: fn(c_uint, Ptr, Ptr) -> c_uint;
    g_idle_add: fn(Ptr, Ptr) -> c_uint;
    g_log_set_writer_func: fn(Ptr, Ptr, Ptr);
    g_log_writer_standard_streams: fn(c_int, Ptr, usize, Ptr) -> c_int;
    g_unix_input_stream_new: fn(c_int, c_int) -> Ptr;
    g_object_class_find_property: fn(Ptr, Text) -> Ptr;
    g_type_query: fn(GType, *mut [u32; 6]);
    g_signal_connect_data: fn(Ptr, Text, Option<Callback>, Ptr, Ptr, c_uint) -> c_ulong;
    g_object_unref: fn(Ptr);
    g_object_ref: fn(Ptr) -> Ptr;
    g_type_class_ref: fn(GType) -> Ptr;
    g_type_check_instance_is_a: fn(Ptr, GType) -> c_int;
    g_bytes_new: fn(*const c_void, usize) -> Ptr;
    g_bytes_get_data: fn(Ptr, *mut usize) -> *const u8;
    g_bytes_unref: fn(Ptr);
    g_error_free: fn(Ptr);
    g_free: fn(Ptr);
    g_error_new_literal: fn(u32, c_int, Text) -> Ptr;
    g_quark_from_static_string: fn(Text) -> u32;
    webkit_get_major_version: fn() -> c_uint;
    webkit_get_minor_version: fn() -> c_uint;
    webkit_web_view_get_type: fn() -> GType;
    webkit_website_policies_get_type: fn() -> GType;
    webkit_user_content_manager_remove_all_scripts: fn(Ptr);
    webkit_web_context_get_type: fn() -> GType;
    webkit_web_view_get_wpe_view: fn(Ptr) -> Ptr;
    webkit_web_context_set_preferred_languages: fn(Ptr, *const Text);
    webkit_web_view_load_uri: fn(Ptr, Text);
    webkit_web_view_evaluate_javascript: fn(Ptr, Text, isize, Text, Text, Ptr, Ptr, Ptr);
    webkit_web_view_evaluate_javascript_finish: fn(Ptr, Ptr, *mut Ptr) -> Ptr;
    webkit_web_view_set_is_muted: fn(Ptr, c_int);
    webkit_web_view_terminate_web_process: fn(Ptr);
    webkit_web_view_get_settings: fn(Ptr) -> Ptr;
    webkit_web_view_set_background_color: fn(Ptr, *const [f64; 4]);
    webkit_web_context_register_uri_scheme: fn(Ptr, Text, Ptr, Ptr, Ptr);
    webkit_web_context_get_security_manager: fn(Ptr) -> Ptr;
    webkit_security_manager_register_uri_scheme_as_secure: fn(Ptr, Text);
    webkit_security_manager_register_uri_scheme_as_cors_enabled: fn(Ptr, Text);
    webkit_uri_scheme_request_get_uri: fn(Ptr) -> Text;
    webkit_uri_scheme_request_finish_with_response: fn(Ptr, Ptr);
    webkit_uri_scheme_request_finish_error: fn(Ptr, Ptr);
    webkit_uri_scheme_response_new: fn(Ptr, i64) -> Ptr;
    webkit_uri_scheme_response_set_content_type: fn(Ptr, Text);
    webkit_uri_scheme_response_set_http_headers: fn(Ptr, Ptr);
    soup_message_headers_new: fn(c_int) -> Ptr;
    soup_message_headers_append: fn(Ptr, Text, Text);
    webkit_user_content_manager_new: fn() -> Ptr;
    webkit_user_content_manager_add_script: fn(Ptr, Ptr);
    webkit_user_content_manager_register_script_message_handler: fn(Ptr, Text, Text) -> c_int;
    webkit_user_content_manager_add_filter: fn(Ptr, Ptr);
    webkit_user_script_new: fn(Text, c_int, c_int, *const Text, *const Text) -> Ptr;
    webkit_user_script_unref: fn(Ptr);
    webkit_user_content_filter_store_new: fn(Text) -> Ptr;
    webkit_user_content_filter_store_save: fn(Ptr, Text, Ptr, Ptr, Ptr, Ptr);
    webkit_user_content_filter_store_save_finish: fn(Ptr, Ptr, *mut Ptr) -> Ptr;
    webkit_network_session_new_ephemeral: fn() -> Ptr;
    webkit_network_session_new: fn(Text, Text) -> Ptr;
    webkit_memory_pressure_settings_new: fn() -> Ptr;
    webkit_memory_pressure_settings_set_memory_limit: fn(Ptr, c_uint);
    webkit_permission_request_deny: fn(Ptr);
    webkit_policy_decision_ignore: fn(Ptr);
    webkit_policy_decision_use: fn(Ptr);
    webkit_navigation_policy_decision_get_navigation_action: fn(Ptr) -> Ptr;
    webkit_navigation_action_get_request: fn(Ptr) -> Ptr;
    webkit_uri_request_get_uri: fn(Ptr) -> Text;
    webkit_download_cancel: fn(Ptr);
    jsc_value_to_string: fn(Ptr) -> *mut c_char;
    wpe_display_headless_new: fn() -> Ptr;
    wpe_display_headless_new_for_device: fn(Text, *mut Ptr) -> Ptr;
    wpe_display_connect: fn(Ptr, *mut Ptr) -> c_int;
    wpe_view_headless_get_type: fn() -> GType;
    wpe_display_headless_get_type: fn() -> GType;
    wpe_display_get_drm_device: fn(Ptr) -> Ptr;
    wpe_buffer_formats_builder_new: fn(Ptr) -> Ptr;
    wpe_buffer_formats_builder_append_group: fn(Ptr, Ptr, c_int);
    wpe_buffer_formats_builder_append_format: fn(Ptr, u32, u64);
    wpe_buffer_formats_builder_end: fn(Ptr) -> Ptr;
    wpe_view_get_toplevel: fn(Ptr) -> Ptr;
    wpe_toplevel_resize: fn(Ptr, c_int, c_int) -> c_int;
    wpe_toplevel_scale_changed: fn(Ptr, f64);
    wpe_view_set_visible: fn(Ptr, c_int);
    wpe_view_buffer_rendered: fn(Ptr, Ptr);
    wpe_view_buffer_released: fn(Ptr, Ptr);
    wpe_view_event: fn(Ptr, Ptr);
    wpe_event_pointer_move_new: fn(c_uint, Ptr, c_uint, u32, c_uint, f64, f64, f64, f64) -> Ptr;
    wpe_event_pointer_button_new: fn(c_uint, Ptr, c_uint, u32, c_uint, c_uint, f64, f64, c_uint) -> Ptr;
    wpe_event_unref: fn(Ptr);
    wpe_buffer_get_width: fn(Ptr) -> c_int;
    wpe_buffer_get_height: fn(Ptr) -> c_int;
    wpe_buffer_import_to_pixels: fn(Ptr, *mut Ptr) -> Ptr;
    wpe_buffer_shm_get_data: fn(Ptr) -> Ptr;
    wpe_buffer_shm_get_stride: fn(Ptr) -> c_uint;
    wpe_buffer_dma_buf_get_type: fn() -> GType;
    wpe_buffer_dma_buf_get_format: fn(Ptr) -> u32;
    wpe_buffer_dma_buf_get_n_planes: fn(Ptr) -> u32;
    wpe_buffer_dma_buf_get_fd: fn(Ptr, u32) -> c_int;
    wpe_buffer_dma_buf_get_offset: fn(Ptr, u32) -> u32;
    wpe_buffer_dma_buf_get_stride: fn(Ptr, u32) -> u32;
    wpe_buffer_dma_buf_get_modifier: fn(Ptr) -> u64;
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

pub fn api() -> Result<&'static Api, String> {
    let loaded = API.get_or_init(|| {
        let lib = unsafe { libloading::Library::new(LIBRARY) }.map_err(|e| e.to_string())?;
        unsafe { Api::load(lib) }.map_err(|name| format!("{LIBRARY} has no {name}"))
    });
    loaded.as_ref().map_err(Clone::clone)
}

pub fn get() -> &'static Api {
    api().expect("loaded at start")
}

pub fn text(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s.replace('\0', "")).unwrap_or_default()
}
