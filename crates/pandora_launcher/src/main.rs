#![deny(unused_must_use)]

use std::sync::Arc;

pub mod panic;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let panic_message = Default::default();

    crate::panic::install_hook(Arc::clone(&panic_message));

    let (backend_recv, backend_handle, frontend_recv, frontend_handle) = bridge::handle::create_pair();

    backend::start(frontend_handle, backend_handle.clone(), backend_recv);
    frontend::start(panic_message, backend_handle, frontend_recv);
}
