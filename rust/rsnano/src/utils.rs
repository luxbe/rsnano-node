use anyhow::Result;
use blake2::digest::{Update, VariableOutput};

pub trait Stream {
    fn write_u8(&mut self, value: u8) -> anyhow::Result<()>;
    fn write_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()>;
    fn read_u8(&mut self) -> anyhow::Result<u8>;
    fn read_bytes(&mut self, buffer: &mut [u8], len: usize) -> anyhow::Result<()>;
}

pub trait Blake2b {
    fn init(&mut self, outlen: usize) -> Result<()>;
    fn update(&mut self, bytes: &[u8]) -> Result<()>;
    fn finalize(&mut self, out: &mut [u8]) -> Result<()>;
}

pub struct RustBlake2b {
    instance: Option<blake2::VarBlake2b>,
}

impl RustBlake2b {
    pub fn new() -> Self {
        Self { instance: None }
    }
}

impl Blake2b for RustBlake2b {
    fn init(&mut self, outlen: usize) -> Result<()> {
        self.instance = Some(blake2::VarBlake2b::new_keyed(&[], outlen));
        Ok(())
    }

    fn update(&mut self, bytes: &[u8]) -> Result<()> {
        self.instance
            .as_mut()
            .ok_or_else(|| anyhow!("not initialized"))?
            .update(bytes);
        Ok(())
    }

    fn finalize(&mut self, out: &mut [u8]) -> Result<()> {
        let i = self
            .instance
            .take()
            .ok_or_else(|| anyhow!("not initialized"))?;

        if out.len() != i.output_size() {
            return Err(anyhow!("output size does not match"));
        }

        i.finalize_variable(|bytes| {
            out.copy_from_slice(bytes);
        });
        Ok(())
    }
}