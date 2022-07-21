#[derive(Clone)]
pub struct GenerateCache {
    pub reps: bool,
    pub cemented_count: bool,
    pub unchecked_count: bool,
    pub account_count: bool,
    pub block_count: bool,
}

impl GenerateCache {
    pub fn new() -> Self {
        Self {
            reps: true,
            cemented_count: true,
            unchecked_count: true,
            account_count: true,
            block_count: true,
        }
    }

    pub fn enable_all(&mut self) {
        self.reps = true;
        self.cemented_count = true;
        self.unchecked_count = true;
        self.account_count = true;
    }
}