use std::vec;

use qftp::auth::{AuthManager, FileStorage};

#[tokio::main]
async fn main() {
    let file_storage = FileStorage::new("./qftp/auth.json").await.unwrap();
    let mut manager = AuthManager::new(file_storage);
    manager
        .add_user("test_user".to_string(), "123".to_string(), 501, vec![20])
        .await
        .unwrap();
}
