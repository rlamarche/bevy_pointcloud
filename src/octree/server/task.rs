use bevy_tasks::prelude::*;
#[cfg(feature = "potree_wasm_worker")]
use bevy_log::prelude::*;

#[cfg(not(feature = "potree_wasm_worker"))]
pub fn spawn_async_task<T>(
    future: impl Future<Output = T> + Send + 'static,
) -> bevy_tasks::Task<T>
where
    T: Send + 'static,
{
    IoTaskPool::get().spawn(future)
}

#[cfg(feature = "potree_wasm_worker")]
pub fn spawn_async_task(
    future: impl Future<Output = ()> + Send + 'static,
) -> () {
    wasm_thread::spawn({
        || {
            info!("Hello from wasm thread!");
            wasm_bindgen_futures::spawn_local(future);

            wasm_bindgen::throw_str(
                "Cursed hack to keep workers alive. See https://github.com/rustwasm/wasm-bindgen/issues/2945",
            );
        }
    });
}
