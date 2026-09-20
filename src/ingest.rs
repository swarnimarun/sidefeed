use crate::AppState;

pub async fn poll_loop(_state: AppState) {
    std::future::pending::<()>().await;
}

