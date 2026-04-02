use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/users", get(list_users_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on http://{}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
        .await
        .unwrap();
}

async fn root_handler() -> &'static str {
    "User service root — up and running!"
}

async fn list_users_handler() -> &'static str {
    "List of users (not implemented)"
}