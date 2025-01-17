use std::ffi::c_void;

pub trait IoContext: Send + Sync {
    fn post(&self, f: Box<dyn FnOnce()>);
    fn raw_handle(&self) -> *mut c_void;
}

#[cfg(test)]
pub struct StubIoContext {}

#[cfg(test)]
impl StubIoContext {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
impl IoContext for StubIoContext {
    fn post(&self, f: Box<dyn FnOnce()>) {
        f();
    }

    fn raw_handle(&self) -> *mut c_void {
        todo!()
    }
}
