#[derive(Debug, Clone)]
pub struct SystemConfig {
    pub qvm_move_path: String,
}

impl SystemConfig {
    pub fn from_env() -> Self {
        let qvm_move_path =
            std::env::var("QVM_MOVE_PATH").unwrap_or_else(|_| "/usr/bin/qvm-move".to_string());

        Self { qvm_move_path }
    }
}
