use crate::raw_calls;

pub struct JoinHandle<T> {
    task_id: usize,
    thread_id: usize,
    data: core::marker::PhantomData<T>
}

impl<T> JoinHandle<T> {
    pub fn join(&self) -> &'static mut T {
        let ptr = raw_calls::await_task_end(self.task_id, self.thread_id);

        unsafe {&mut *ptr}
    }
}

/// Safely spawns a thread, ensuring it is dropped at the end of it's lifetime
pub fn spawn_thread<T>(f: fn(usize, usize) -> T) -> JoinHandle<T> {
    let ids = unsafe {raw_calls::spawn_thread_raw()};

    if ids.2 {
        let boxed = crate::boxed::Box::new(f(ids.0, ids.1));
        let leaked = crate::boxed::Box::leak(boxed);
        unsafe {raw_calls::drop_thread(leaked)};
    } else {
        JoinHandle {
            task_id: ids.0,
            thread_id: ids.1,
            data: core::marker::PhantomData
        }
    }
}