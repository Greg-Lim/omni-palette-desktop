pub mod backend_contract;
pub mod config;
pub mod core;
pub mod domain;
pub mod platform {
    pub mod hotkey_actions;
    pub mod platform_interface;

    #[cfg(target_os = "windows")]
    pub mod windows {
        pub mod context;
        pub mod mapper;
        pub mod receiver;
        pub mod sender;
    }
}
